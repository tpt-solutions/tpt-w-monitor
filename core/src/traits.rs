// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The interfaces every platform backend implements.

use crate::action::ProcessAction;
use crate::error::Result;
use crate::model::{Pid, ProcessMetrics, SystemMetrics};
use crate::sensors::{FanReading, TemperatureReading};

/// Enumerates processes, samples metrics, and applies process actions.
///
/// Backends are stateful (rates are computed as deltas between samples), hence
/// the `&mut self` receivers.
pub trait SystemMonitor {
    /// Whether this backend can run on the current system. Backends that are
    /// design-only (e.g. the Archon stub) return `false`.
    fn is_available(&self) -> bool {
        true
    }

    /// Samples metrics for every visible process.
    fn sample_processes(&mut self) -> Result<Vec<ProcessMetrics>>;

    /// Samples system-wide and per-component metrics.
    fn sample_system(&mut self) -> Result<SystemMetrics>;

    /// Applies an already-validated action. Callers should go through
    /// [`crate::execute_action`], which enforces the confirmation gate.
    fn apply_action(&mut self, pid: Pid, action: ProcessAction) -> Result<()>;
}

/// Hardware sensor access. Every method returns `None` when the sensor path is
/// unavailable for the current hardware/OS, and `Some(vec![])` when the path
/// works but reports nothing.
pub trait SensorProvider {
    /// Temperature readings, or `None` if unavailable.
    fn temperatures(&self) -> Option<Vec<TemperatureReading>>;

    /// Fan speed readings, or `None` if unavailable.
    fn fans(&self) -> Option<Vec<FanReading>>;
}
