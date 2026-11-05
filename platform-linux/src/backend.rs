// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Linux backend: reads `/proc` and `/sys`, acts on processes with `libc`.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::{Duration, Instant};

use tpt_monitor_core::{
    FanReading, MonitorError, Pid, Priority, ProcessAction, ProcessMetrics, Result, SensorProvider,
    SystemMetrics, SystemMonitor, TemperatureReading,
};

use crate::parse;

/// Counters remembered from the previous sample for one process.
#[derive(Debug, Clone, Copy)]
struct PrevProcess {
    start_ticks: u64,
    busy_ticks: u64,
    io: Option<(u64, u64)>,
}

/// Linux backend. Keeps the previous sample so cumulative counters become rates.
pub struct LinuxMonitor {
    cores: usize,
    clk_tck: u64,
    last_sample: Option<Instant>,
    prev_processes: HashMap<u32, PrevProcess>,
    prev_cpu: Option<parse::CpuTimes>,
}

impl Default for LinuxMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxMonitor {
    /// Creates a backend. Core count and clock-tick rate come from the kernel.
    pub fn new() -> Self {
        // SAFETY: sysconf takes a constant name and has no memory preconditions.
        let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
        // SAFETY: as above, for the online-processor query.
        let cores = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) }.max(1) as usize;
        Self { cores, clk_tck, last_sample: None, prev_processes: HashMap::new(), prev_cpu: None }
    }

    /// Logical processor count used for CPU scaling.
    pub fn logical_cores(&self) -> usize {
        self.cores
    }
}

/// Converts a tick delta into whole-machine percent.
fn process_cpu_percent(delta_ticks: u64, clk_tck: u64, elapsed: Duration, cores: usize) -> f32 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    let busy_secs = delta_ticks as f64 / clk_tck as f64;
    (busy_secs / (secs * cores as f64) * 100.0).clamp(0.0, 100.0) as f32
}

fn rate(delta: u64, elapsed: Duration) -> u64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 { 0 } else { (delta as f64 / secs) as u64 }
}

/// Reads the numeric PIDs under `/proc`.
fn list_pids() -> Result<Vec<u32>> {
    let entries = fs::read_dir("/proc").map_err(|e| MonitorError::Os(e.to_string()))?;
    Ok(entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .collect())
}

impl SystemMonitor for LinuxMonitor {
    fn sample_processes(&mut self) -> Result<Vec<ProcessMetrics>> {
        let now = Instant::now();
        let elapsed = self.last_sample.map(|t| now.duration_since(t));
        let mut next = HashMap::new();
        let mut out = Vec::new();

        for raw_pid in list_pids()? {
            // The process may exit between readdir and read; skip it silently.
            let Ok(stat_text) = fs::read_to_string(format!("/proc/{raw_pid}/stat")) else {
                continue;
            };
            let Some(stat) = parse::parse_proc_stat(&stat_text) else {
                continue;
            };

            let pid = Pid(raw_pid);
            let mut metrics = ProcessMetrics::new(pid, stat.name);
            // No window detection on Linux yet. Treat every process as visible so the
            // suggestion engine never proposes actions it cannot justify.
            metrics.has_visible_window = true;

            let prev = self
                .prev_processes
                .get(&raw_pid)
                .filter(|p| p.start_ticks == stat.start_ticks)
                .copied();
            if let (Some(prev), Some(elapsed)) = (prev, elapsed) {
                metrics.cpu_percent = process_cpu_percent(
                    stat.busy_ticks.saturating_sub(prev.busy_ticks),
                    self.clk_tck,
                    elapsed,
                    self.cores,
                );
            }

            // /proc/<pid>/io is readable only for same-user processes (or as root).
            let io = fs::read_to_string(format!("/proc/{raw_pid}/io"))
                .ok()
                .and_then(|t| parse::parse_proc_io(&t));
            if let (Some((read, write)), Some(prev_io), Some(elapsed)) =
                (io, prev.and_then(|p| p.io), elapsed)
            {
                metrics.disk_read_bytes_per_sec =
                    Some(rate(read.saturating_sub(prev_io.0), elapsed));
                metrics.disk_write_bytes_per_sec =
                    Some(rate(write.saturating_sub(prev_io.1), elapsed));
            }

            if let Ok(text) = fs::read_to_string(format!("/proc/{raw_pid}/status")) {
                let status = parse::parse_proc_status(&text);
                metrics.memory.working_set_bytes = status.rss_bytes;
                metrics.memory.private_bytes = status.anon_bytes;
                metrics.memory.commit_bytes = status.vm_size_bytes;
            }

            next.insert(
                raw_pid,
                PrevProcess { start_ticks: stat.start_ticks, busy_ticks: stat.busy_ticks, io },
            );
            out.push(metrics);
        }

        self.prev_processes = next;
        self.last_sample = Some(now);
        Ok(out)
    }

    fn sample_system(&mut self) -> Result<SystemMetrics> {
        let meminfo =
            fs::read_to_string("/proc/meminfo").map_err(|e| MonitorError::Os(e.to_string()))?;
        let (total, available) = parse::parse_meminfo(&meminfo)
            .ok_or_else(|| MonitorError::Os("unparseable /proc/meminfo".into()))?;

        let stat = fs::read_to_string("/proc/stat").map_err(|e| MonitorError::Os(e.to_string()))?;
        let times = parse::parse_proc_stat_cpu(&stat)
            .ok_or_else(|| MonitorError::Os("unparseable /proc/stat".into()))?;

        let mut system = SystemMetrics {
            memory_total_bytes: total,
            memory_used_bytes: total.saturating_sub(available),
            // Per-drive, network and GPU breakdowns come in later phases.
            ..SystemMetrics::default()
        };

        if let Some(prev) = &self.prev_cpu {
            system.cpu_total_percent = busy_percent(prev.total, times.total);
            system.cpu_per_core_percent = times
                .per_core
                .iter()
                .zip(prev.per_core.iter())
                .map(|(now, before)| busy_percent(*before, *now))
                .collect();
        } else {
            system.cpu_per_core_percent = vec![0.0; times.per_core.len()];
        }
        self.prev_cpu = Some(times);
        Ok(system)
    }

    fn apply_action(&mut self, pid: Pid, action: ProcessAction) -> Result<()> {
        action.validate()?;
        let raw = pid.0 as libc::pid_t;
        match action {
            ProcessAction::Kill => signal(pid, raw, libc::SIGKILL),
            ProcessAction::Suspend => signal(pid, raw, libc::SIGSTOP),
            ProcessAction::Resume => signal(pid, raw, libc::SIGCONT),
            ProcessAction::SetPriority(p) => {
                // SAFETY: setpriority reads plain integers only.
                let rc = unsafe {
                    libc::setpriority(libc::PRIO_PROCESS, pid.0 as libc::id_t, nice_for(p))
                };
                check(rc, pid)
            }
            ProcessAction::SetAffinity(mask) => set_affinity(pid, mask),
        }
    }
}

/// CPU busy percent between two `(total, idle)` tick readings.
fn busy_percent(before: (u64, u64), now: (u64, u64)) -> f32 {
    let total = now.0.saturating_sub(before.0);
    let idle = now.1.saturating_sub(before.1);
    if total == 0 {
        return 0.0;
    }
    ((total.saturating_sub(idle)) as f64 / total as f64 * 100.0) as f32
}

/// Nice value for a priority class. Negative values need CAP_SYS_NICE.
fn nice_for(p: Priority) -> libc::c_int {
    match p {
        Priority::Idle => 19,
        Priority::BelowNormal => 10,
        Priority::Normal => 0,
        Priority::AboveNormal => -5,
        Priority::High => -10,
    }
}

fn signal(pid: Pid, raw: libc::pid_t, sig: libc::c_int) -> Result<()> {
    // SAFETY: kill takes plain integers; no memory is shared.
    let rc = unsafe { libc::kill(raw, sig) };
    check(rc, pid)
}

fn set_affinity(pid: Pid, mask: u64) -> Result<()> {
    // SAFETY: a zeroed cpu_set_t is a valid empty set.
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    for cpu in 0..64usize {
        if mask & (1u64 << cpu) != 0 {
            // SAFETY: `cpu` is below the set size; CPU_SET only writes inside `set`.
            unsafe { libc::CPU_SET(cpu, &mut set) };
        }
    }
    // SAFETY: `set` is a valid, initialised cpu_set_t for the duration of the call.
    let rc = unsafe {
        libc::sched_setaffinity(pid.0 as libc::pid_t, std::mem::size_of::<libc::cpu_set_t>(), &set)
    };
    check(rc, pid)
}

fn check(rc: libc::c_int, pid: Pid) -> Result<()> {
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    Err(match err.raw_os_error() {
        Some(libc::ESRCH) => MonitorError::ProcessNotFound(pid),
        Some(libc::EPERM) | Some(libc::EACCES) => MonitorError::PermissionDenied(pid),
        _ => MonitorError::Os(format!("pid {pid}: {err}")),
    })
}

/// Sensors from `/sys/class/hwmon`. Missing hwmon support yields `None`, and an
/// hwmon tree with no matching files yields `Some(vec![])`.
pub struct LinuxSensors;

impl LinuxSensors {
    /// Creates the sensor reader. It holds no state; each call reads sysfs.
    pub fn new() -> Self {
        Self
    }

    /// Reads every `<prefix><n>_input` file under every hwmon device.
    fn read_inputs(prefix: &str) -> Option<Vec<(String, i64)>> {
        let devices = fs::read_dir("/sys/class/hwmon").ok()?;
        let mut out = Vec::new();
        for dev in devices.filter_map(|d| d.ok()) {
            let path = dev.path();
            let device_name = fs::read_to_string(path.join("name"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| dev.file_name().to_string_lossy().into_owned());
            let Ok(files) = fs::read_dir(&path) else { continue };
            for f in files.filter_map(|f| f.ok()) {
                let fname = f.file_name().to_string_lossy().into_owned();
                let Some(idx) = fname.strip_prefix(prefix).and_then(|r| r.strip_suffix("_input"))
                else {
                    continue;
                };
                let Some(value) =
                    fs::read_to_string(f.path()).ok().and_then(|t| parse::parse_sysfs_int(&t))
                else {
                    continue;
                };
                let label = fs::read_to_string(path.join(format!("{prefix}{idx}_label")))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| format!("{device_name} {prefix}{idx}"));
                out.push((label, value));
            }
        }
        Some(out)
    }
}

impl Default for LinuxSensors {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorProvider for LinuxSensors {
    fn temperatures(&self) -> Option<Vec<TemperatureReading>> {
        // hwmon reports millidegrees Celsius.
        let raw = Self::read_inputs("temp")?;
        Some(
            raw.into_iter()
                .map(|(label, milli)| TemperatureReading { label, celsius: milli as f32 / 1000.0 })
                .collect(),
        )
    }

    fn fans(&self) -> Option<Vec<FanReading>> {
        let raw = Self::read_inputs("fan")?;
        Some(
            raw.into_iter()
                .filter(|(_, rpm)| *rpm >= 0)
                .map(|(label, rpm)| FanReading { label, rpm: rpm as u32 })
                .collect(),
        )
    }
}

/// Whether the app runs as root (effective UID 0). Root can see and control every process.
pub fn is_elevated() -> bool {
    // SAFETY: geteuid takes no arguments and cannot fail.
    unsafe { libc::geteuid() == 0 }
}
