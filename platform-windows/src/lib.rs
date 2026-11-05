// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Windows implementation of [`tpt_monitor_core::SystemMonitor`].
//!
//! Sources:
//! - process list: Toolhelp32 snapshot
//! - per-process CPU: `GetProcessTimes` deltas, scaled to whole-machine percent
//! - per-process memory: `K32GetProcessMemoryInfo`
//! - per-process disk I/O: `GetProcessIoCounters` deltas
//! - visible-window heuristic: `EnumWindows` + `IsWindowVisible`
//! - per-CPU load: `NtQuerySystemInformation(SystemProcessorPerformanceInformation)`
//! - physical memory: `GlobalMemoryStatusEx`
//!
//! The first sample has no previous counters, so CPU and rate fields read 0 and
//! `None` respectively. Callers should discard or de-emphasise that sample.
//!
//! Not yet provided (see todo.md Phase 2): per-process network attribution,
//! per-drive and per-adapter breakdowns, PDH GPU counters, and sensors.

#![cfg(windows)]

pub mod calc;
pub mod gpu;
pub mod net;
mod sys;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use tpt_monitor_core::{
    MonitorError, Pid, ProcessAction, ProcessMetrics, Result, SystemMetrics, SystemMonitor,
};

/// Counters remembered from the previous sample for one process.
#[derive(Debug, Clone, Copy)]
struct PrevProcess {
    /// Creation time, used to detect PID reuse between samples.
    creation: u64,
    /// Cumulative kernel + user ticks.
    busy_ticks: u64,
    /// Cumulative (read, write) bytes, if the counter was readable.
    io: Option<(u64, u64)>,
}

/// Windows backend. Holds the state needed to turn cumulative counters into rates.
pub struct WindowsMonitor {
    cores: usize,
    last_sample: Option<Instant>,
    prev_processes: HashMap<Pid, PrevProcess>,
    prev_cpu: Option<Vec<(u64, u64)>>,
    net: net::NetTracker,
    /// Connections for which byte-counter collection is already switched on.
    estats_enabled: HashSet<net::TcpRow>,
    /// GPU counters, if this machine exposes them.
    gpu: Option<sys::GpuCounters>,
}

impl Default for WindowsMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsMonitor {
    /// Creates a backend and detects the logical core count.
    pub fn new() -> Self {
        Self {
            cores: sys::logical_processor_count(),
            last_sample: None,
            prev_processes: HashMap::new(),
            prev_cpu: None,
            net: net::NetTracker::new(),
            estats_enabled: HashSet::new(),
            gpu: sys::open_gpu_counters().ok(),
        }
    }

    /// Per-process TCP rates for this interval, or `None` if the connection table
    /// cannot be read. Connections whose counters are unavailable (usually because the
    /// app is not elevated) contribute nothing.
    fn sample_net(&mut self, elapsed: Duration) -> Option<HashMap<u32, net::NetRates>> {
        let table = sys::tcp_owner_table().ok()?;
        let rows = net::parse_owner_table(&table);
        let mut samples = Vec::new();
        for row in rows.iter().filter(|r| r.state == net::STATE_ESTABLISHED) {
            if !self.estats_enabled.contains(row) && sys::enable_estats(row).is_ok() {
                self.estats_enabled.insert(*row);
            }
            let counters =
                if self.estats_enabled.contains(row) { sys::estats_counters(row) } else { None };
            samples.push(net::ConnSample { pid: row.pid, key: *row, counters });
        }
        let live: HashSet<net::TcpRow> = rows.iter().copied().collect();
        self.estats_enabled.retain(|row| live.contains(row));
        Some(self.net.observe(&samples, elapsed))
    }

    /// Logical processor count used for CPU scaling.
    pub fn logical_cores(&self) -> usize {
        self.cores
    }
}

/// Whether the app runs elevated, which decides how much of the system it can see and
/// control on Windows.
pub fn is_elevated() -> bool {
    sys::is_elevated()
}

impl SystemMonitor for WindowsMonitor {
    fn user_idle_secs(&self) -> Option<u64> {
        sys::user_idle_seconds()
    }

    fn sample_processes(&mut self) -> Result<Vec<ProcessMetrics>> {
        let now = Instant::now();
        let elapsed = self.last_sample.map(|t| now.duration_since(t));
        let wall_ticks = elapsed.map(calc::elapsed_ticks).unwrap_or(0);
        let windows = sys::pids_with_visible_windows();
        let tcp = self.sample_net(elapsed.unwrap_or(Duration::ZERO));
        let gpu = self.gpu.as_mut().and_then(|g| g.sample());
        let gpu_engine = gpu.as_ref().map(|(engine, _)| gpu::engine_percent_by_pid(engine));
        let gpu_memory = gpu.as_ref().map(|(_, memory)| gpu::memory_by_pid(memory));

        let mut next: HashMap<Pid, PrevProcess> = HashMap::new();
        let mut out = Vec::new();

        for (pid, name) in sys::list_processes()? {
            let mut metrics = ProcessMetrics::new(pid, name);
            metrics.has_visible_window = windows.contains(&pid.0);
            if let Some(engine) = &gpu_engine {
                // Present whenever GPU counters work; a process with no GPU work reads zero.
                metrics.gpu_engine_percent = Some(engine.get(&pid.0).copied().unwrap_or(0.0));
            }
            if let Some(memory) = &gpu_memory {
                metrics.vram_bytes = Some(memory.get(&pid.0).copied().unwrap_or(0));
            }
            if let Some(rates) = &tcp {
                // Only processes with connections appear in the table; the rest read zero.
                let r = rates.get(&pid.0).copied().unwrap_or_default();
                metrics.net_up_bytes_per_sec = Some(r.up);
                metrics.net_down_bytes_per_sec = Some(r.down);
            }

            if let Some(handle) = sys::open_for_query(pid) {
                if let Ok((creation, busy_ticks)) = sys::process_times(&handle) {
                    let prev = self.prev_processes.get(&pid).filter(|p| p.creation == creation);
                    if let (Some(prev), true) = (prev, wall_ticks > 0) {
                        metrics.cpu_percent = calc::process_cpu_percent(
                            calc::counter_delta(prev.busy_ticks, busy_ticks),
                            wall_ticks,
                            self.cores,
                        );
                    }
                    let io = sys::io_counters(&handle).ok();
                    let prev_io = prev.and_then(|p| p.io);
                    if let (Some((read, write)), Some((prev_read, prev_write)), Some(elapsed)) =
                        (io, prev_io, elapsed)
                    {
                        metrics.disk_read_bytes_per_sec = Some(calc::bytes_per_sec(
                            calc::counter_delta(prev_read, read),
                            elapsed,
                        ));
                        metrics.disk_write_bytes_per_sec = Some(calc::bytes_per_sec(
                            calc::counter_delta(prev_write, write),
                            elapsed,
                        ));
                    }
                    if let Ok((ws, private, commit)) = sys::memory_usage(&handle) {
                        metrics.memory.working_set_bytes = ws;
                        metrics.memory.private_bytes = private;
                        metrics.memory.commit_bytes = commit;
                    }
                    next.insert(pid, PrevProcess { creation, busy_ticks, io });
                }
            }
            out.push(metrics);
        }

        self.prev_processes = next;
        self.last_sample = Some(now);
        Ok(out)
    }

    fn sample_system(&mut self) -> Result<SystemMetrics> {
        let (total, available) = sys::physical_memory()?;
        let times = sys::processor_times(self.cores)?;

        let mut system = SystemMetrics {
            memory_total_bytes: total,
            memory_used_bytes: total.saturating_sub(available),
            // Network, disk-per-drive and GPU are filled in by later phases.
            net_up_bytes_per_sec: None,
            net_down_bytes_per_sec: None,
            ..SystemMetrics::default()
        };

        if let Some(prev) = &self.prev_cpu {
            let mut sum_total = 0u64;
            let mut sum_idle = 0u64;
            for (&(total_now, idle_now), &(total_prev, idle_prev)) in times.iter().zip(prev.iter())
            {
                let total_d = calc::counter_delta(total_prev, total_now);
                let idle_d = calc::counter_delta(idle_prev, idle_now);
                system.cpu_per_core_percent.push(calc::cpu_busy_percent(total_d, idle_d));
                sum_total += total_d;
                sum_idle += idle_d;
            }
            system.cpu_total_percent = calc::cpu_busy_percent(sum_total, sum_idle);
        } else {
            system.cpu_per_core_percent = vec![0.0; times.len()];
        }
        self.prev_cpu = Some(times);
        Ok(system)
    }

    fn apply_action(&mut self, pid: Pid, action: ProcessAction) -> Result<()> {
        action.validate()?;
        match action {
            ProcessAction::Kill => {
                let h = sys::open(pid, sys::ACCESS_TERMINATE)?;
                sys::terminate(&h)
            }
            ProcessAction::Suspend => {
                let h = sys::open(pid, sys::ACCESS_SUSPEND_RESUME)?;
                sys::suspend(&h)
            }
            ProcessAction::Resume => {
                let h = sys::open(pid, sys::ACCESS_SUSPEND_RESUME)?;
                sys::resume(&h)
            }
            ProcessAction::SetPriority(p) => {
                let h = sys::open(pid, sys::ACCESS_SET_INFORMATION)?;
                sys::set_priority(&h, sys::priority_class(p))
            }
            ProcessAction::SetAffinity(mask) => {
                let mask = usize::try_from(mask)
                    .map_err(|_| MonitorError::InvalidAction("affinity mask too wide"))?;
                let h = sys::open(pid, sys::ACCESS_SET_INFORMATION)?;
                sys::set_affinity(&h, mask)
            }
        }
    }
}

/// Windows sensor provider. Temperature and fan access is not wired yet: WMI's
/// `MSAcpi_ThermalZoneTemperature` is the first candidate (todo.md Phase 4), and
/// it needs admin rights and often reports nothing. Until that is evaluated, both
/// readings report unavailable (`None`) so the UI shows its "unavailable" state.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsSensors;

impl tpt_monitor_core::SensorProvider for WindowsSensors {
    fn temperatures(&self) -> Option<Vec<tpt_monitor_core::TemperatureReading>> {
        None
    }

    fn fans(&self) -> Option<Vec<tpt_monitor_core::FanReading>> {
        None
    }
}

#[cfg(test)]
mod sensor_tests {
    use super::WindowsSensors;
    use tpt_monitor_core::SensorProvider;

    #[test]
    fn windows_sensors_report_unavailable_not_zero() {
        assert_eq!(WindowsSensors.temperatures(), None);
        assert_eq!(WindowsSensors.fans(), None);
    }
}
