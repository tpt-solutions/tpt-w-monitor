// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Metric data model. Metrics that a platform may not expose are `Option`s so
//! that "unavailable" is distinct from "zero".

use std::fmt;

/// Operating-system process identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid(pub u32);

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Per-process memory, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryUsage {
    /// Resident (working set) memory.
    pub working_set_bytes: u64,
    /// Memory private to the process (not shared).
    pub private_bytes: u64,
    /// Committed virtual memory.
    pub commit_bytes: u64,
}

/// Metrics for a single process at one sample.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessMetrics {
    /// Process identifier.
    pub pid: Pid,
    /// Executable name, e.g. `chrome.exe`.
    pub name: String,
    /// CPU usage over the last sample interval, 0.0..=100.0 per logical core
    /// scaled to total-machine percent.
    pub cpu_percent: f32,
    /// Memory usage.
    pub memory: MemoryUsage,
    /// GPU engine utilisation, if the platform reports it.
    pub gpu_engine_percent: Option<f32>,
    /// VRAM used by this process, if the platform reports it.
    pub vram_bytes: Option<u64>,
    /// Disk read rate, if available.
    pub disk_read_bytes_per_sec: Option<u64>,
    /// Disk write rate, if available.
    pub disk_write_bytes_per_sec: Option<u64>,
    /// Network upload rate, if available.
    pub net_up_bytes_per_sec: Option<u64>,
    /// Network download rate, if available.
    pub net_down_bytes_per_sec: Option<u64>,
    /// Whether the process owns a visible top-level window. Used by the
    /// suggestion heuristics to detect background work.
    pub has_visible_window: bool,
}

impl ProcessMetrics {
    /// Creates metrics for `pid` with every optional metric unset and zeroed usage.
    pub fn new(pid: Pid, name: impl Into<String>) -> Self {
        Self {
            pid,
            name: name.into(),
            cpu_percent: 0.0,
            memory: MemoryUsage::default(),
            gpu_engine_percent: None,
            vram_bytes: None,
            disk_read_bytes_per_sec: None,
            disk_write_bytes_per_sec: None,
            net_up_bytes_per_sec: None,
            net_down_bytes_per_sec: None,
            has_visible_window: false,
        }
    }
}

/// A physical or logical drive.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DriveMetrics {
    /// Drive label, e.g. `C:` or `/dev/nvme0n1`.
    pub name: String,
    /// Read rate, if available.
    pub read_bytes_per_sec: Option<u64>,
    /// Write rate, if available.
    pub write_bytes_per_sec: Option<u64>,
}

/// A GPU adapter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdapterMetrics {
    /// Adapter name.
    pub name: String,
    /// Overall engine utilisation, if available.
    pub engine_percent: Option<f32>,
    /// VRAM in use, if available.
    pub vram_used_bytes: Option<u64>,
    /// VRAM capacity, if available.
    pub vram_total_bytes: Option<u64>,
}

/// System-wide metrics at one sample.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemMetrics {
    /// Total CPU utilisation, 0.0..=100.0.
    pub cpu_total_percent: f32,
    /// Per-logical-core CPU utilisation.
    pub cpu_per_core_percent: Vec<f32>,
    /// Installed physical memory.
    pub memory_total_bytes: u64,
    /// Physical memory in use.
    pub memory_used_bytes: u64,
    /// Per-drive I/O.
    pub drives: Vec<DriveMetrics>,
    /// Per-adapter GPU metrics.
    pub adapters: Vec<AdapterMetrics>,
    /// Total network upload rate.
    /// Total network upload rate, `None` until a backend provides it.
    pub net_up_bytes_per_sec: Option<u64>,
    /// Total network download rate, `None` until a backend provides it.
    pub net_down_bytes_per_sec: Option<u64>,
}

impl SystemMetrics {
    /// Fraction of physical memory in use, 0.0..=1.0. Returns 0.0 when the
    /// total is unknown (zero) rather than dividing by zero.
    pub fn memory_used_fraction(&self) -> f32 {
        if self.memory_total_bytes == 0 {
            return 0.0;
        }
        (self.memory_used_bytes as f64 / self.memory_total_bytes as f64).min(1.0) as f32
    }
}
