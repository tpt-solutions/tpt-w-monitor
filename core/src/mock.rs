// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! In-memory backends for tests and for developing the UI without a real OS.

use crate::action::ProcessAction;
use crate::error::{MonitorError, Result};
use crate::model::{Pid, ProcessMetrics, SystemMetrics};
use crate::sensors::{FanReading, TemperatureReading};
use crate::traits::{SensorProvider, SystemMonitor};

/// A scripted [`SystemMonitor`]. Tests set the processes and system metrics,
/// then inspect `applied` to see which actions reached the backend.
#[derive(Debug, Default, Clone)]
pub struct MockMonitor {
    /// Processes returned by [`SystemMonitor::sample_processes`].
    pub processes: Vec<ProcessMetrics>,
    /// System metrics returned by [`SystemMonitor::sample_system`].
    pub system: SystemMetrics,
    /// Every action that reached the backend, in order.
    pub applied: Vec<(Pid, ProcessAction)>,
    /// If set, every action fails with this error.
    pub fail_actions_with: Option<MonitorError>,
    /// Value returned by [`SystemMonitor::is_available`].
    pub available: bool,
    /// Value returned by [`SystemMonitor::user_idle_secs`].
    pub idle_secs: Option<u64>,
}

impl MockMonitor {
    /// Creates an available mock with no processes.
    pub fn new() -> Self {
        Self { available: true, ..Self::default() }
    }
}

impl SystemMonitor for MockMonitor {
    fn is_available(&self) -> bool {
        self.available
    }

    fn user_idle_secs(&self) -> Option<u64> {
        self.idle_secs
    }

    fn sample_processes(&mut self) -> Result<Vec<ProcessMetrics>> {
        Ok(self.processes.clone())
    }

    fn sample_system(&mut self) -> Result<SystemMetrics> {
        Ok(self.system.clone())
    }

    fn apply_action(&mut self, pid: Pid, action: ProcessAction) -> Result<()> {
        if let Some(err) = &self.fail_actions_with {
            return Err(err.clone());
        }
        if !self.processes.iter().any(|p| p.pid == pid) {
            return Err(MonitorError::ProcessNotFound(pid));
        }
        self.applied.push((pid, action));
        if action == ProcessAction::Kill {
            self.processes.retain(|p| p.pid != pid);
        }
        Ok(())
    }
}

/// A fixed [`SensorProvider`]. `None` fields simulate unavailable hardware.
#[derive(Debug, Default, Clone)]
pub struct MockSensors {
    /// Temperature readings, or `None` for unavailable.
    pub temperatures: Option<Vec<TemperatureReading>>,
    /// Fan readings, or `None` for unavailable.
    pub fans: Option<Vec<FanReading>>,
}

impl SensorProvider for MockSensors {
    fn temperatures(&self) -> Option<Vec<TemperatureReading>> {
        self.temperatures.clone()
    }

    fn fans(&self) -> Option<Vec<FanReading>> {
        self.fans.clone()
    }
}
