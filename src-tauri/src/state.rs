// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Backend state behind the IPC layer. Generic over the monitor so the boundary
//! rules can be tested with the core's mock backend, without a window.
//!
//! Rules enforced here, not in the UI:
//! - an action may only target a PID present in the latest snapshot;
//! - kill and suspend are refused for critical system processes;
//! - kill runs only when the caller confirms it.

use std::collections::HashSet;
use std::time::Duration;

use tpt_monitor_core::{
    CriticalList, FanReading, MonitorError, Pid, ProcessAction, Result, Sampler, SamplingScheduler,
    SensorProvider, Snapshot, Suggestion, SuggestionConfig, SuggestionEngine, SystemMonitor,
    TemperatureReading, ViewState, execute_action,
};

/// Samples kept per series for the live graphs.
pub const HISTORY_SAMPLES: usize = 120;

/// One published frame: the snapshot plus the current suggestion feed.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The sample this frame was built from.
    pub snapshot: Snapshot,
    /// Suggestions, highest impact first.
    pub suggestions: Vec<Suggestion>,
    /// Sensor readings; each part is `None` when the sensor path is unavailable.
    pub sensors: SensorReadings,
}

/// Temperature and fan readings for one frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SensorReadings {
    /// Temperatures, or `None` if unavailable on this hardware/OS.
    pub temperatures: Option<Vec<TemperatureReading>>,
    /// Fan speeds, or `None` if unavailable on this hardware/OS.
    pub fans: Option<Vec<FanReading>>,
}

/// Owns the sampler, the suggestion engine, and the set of PIDs the UI may act on.
pub struct MonitorState<M: SystemMonitor> {
    sampler: Sampler<M>,
    sensors: Box<dyn SensorProvider + Send>,
    suggestions: SuggestionEngine,
    latest: Option<Frame>,
    known_pids: HashSet<Pid>,
}

impl<M: SystemMonitor> MonitorState<M> {
    /// Builds the state with the default scheduler (1 s base, 8 s cap) and rule thresholds.
    pub fn new(
        monitor: M,
        critical: CriticalList,
        sensors: Box<dyn SensorProvider + Send>,
    ) -> Self {
        let scheduler = SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(8));
        Self {
            sampler: Sampler::new(monitor, scheduler, HISTORY_SAMPLES),
            sensors,
            suggestions: SuggestionEngine::new(SuggestionConfig::default(), critical),
            latest: None,
            known_pids: HashSet::new(),
        }
    }

    /// Takes one sample, updates suggestions and the known-PID set, and returns the frame.
    pub fn tick(&mut self) -> Result<&Frame> {
        let snapshot = self.sampler.sample()?;
        self.suggestions.observe(&snapshot.processes);
        self.known_pids = snapshot.processes.iter().map(|p| p.pid).collect();
        let suggestions = self.suggestions.suggestions();
        let sensors =
            SensorReadings { temperatures: self.sensors.temperatures(), fans: self.sensors.fans() };
        self.latest = Some(Frame { snapshot, suggestions, sensors });
        Ok(self.latest.as_ref().expect("frame was just stored"))
    }

    /// The most recent frame, if any sample has been taken.
    pub fn latest(&self) -> Option<&Frame> {
        self.latest.as_ref()
    }

    /// Recent system CPU samples, oldest first, for the tray graph.
    pub fn cpu_history(&self) -> Vec<f32> {
        self.sampler.system_cpu_history().iter().copied().collect()
    }

    /// The `n` busiest processes in the latest frame, as (name, CPU percent).
    pub fn top_processes(&self, n: usize) -> Vec<(String, f32)> {
        let Some(frame) = self.latest.as_ref() else {
            return Vec::new();
        };
        let mut all: Vec<(String, f32)> =
            frame.snapshot.processes.iter().map(|p| (p.name.clone(), p.cpu_percent)).collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        all.truncate(n);
        all
    }

    /// Time to wait before the next [`Self::tick`].
    pub fn next_interval(&self) -> Duration {
        self.sampler.next_interval()
    }

    /// Updates what the user can see, which changes the sampling rate.
    pub fn set_view_state(&mut self, view: ViewState) {
        self.sampler.scheduler_mut().set_view_state(view);
    }

    /// Validates and applies an action requested over IPC.
    pub fn apply_action(&mut self, pid: Pid, action: ProcessAction, confirmed: bool) -> Result<()> {
        action.validate()?;
        if !self.known_pids.contains(&pid) {
            return Err(MonitorError::ProcessNotFound(pid));
        }
        if let Some(name) = self.name_of(pid) {
            let protected = self.suggestions.critical_list().is_critical(&name);
            let disruptive = matches!(action, ProcessAction::Kill | ProcessAction::Suspend);
            if protected && disruptive {
                return Err(MonitorError::InvalidAction(
                    "critical system processes cannot be killed or suspended",
                ));
            }
        }
        execute_action(self.sampler.monitor_mut(), pid, action, confirmed)
    }

    /// Stops suggestions for an executable.
    pub fn dismiss_suggestion(&mut self, process_name: &str) {
        self.suggestions.dismiss(process_name);
    }

    /// Re-enables suggestions for an executable.
    pub fn restore_suggestion(&mut self, process_name: &str) {
        self.suggestions.restore(process_name);
    }

    fn name_of(&self, pid: Pid) -> Option<String> {
        self.latest
            .as_ref()?
            .snapshot
            .processes
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| p.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_monitor_core::ProcessMetrics;
    use tpt_monitor_core::mock::{MockMonitor, MockSensors};

    #[test]
    fn top_processes_are_sorted_and_limited() {
        let mut mock = MockMonitor::new();
        for (pid, name, cpu) in [(1, "idle.exe", 1.0), (2, "hot.exe", 80.0), (3, "warm.exe", 20.0)]
        {
            let mut p = ProcessMetrics::new(Pid(pid), name);
            p.cpu_percent = cpu;
            mock.processes.push(p);
        }
        let mut s = MonitorState::new(
            mock,
            CriticalList::windows_default(),
            Box::new(MockSensors::default()),
        );
        assert!(s.top_processes(2).is_empty(), "no frame yet");
        s.tick().unwrap();
        let top = s.top_processes(2);
        assert_eq!(top, vec![("hot.exe".to_string(), 80.0), ("warm.exe".to_string(), 20.0)]);
    }

    #[test]
    fn cpu_history_grows_with_each_tick() {
        let mut mock = MockMonitor::new();
        mock.system.cpu_total_percent = 42.0;
        let mut s = MonitorState::new(
            mock,
            CriticalList::windows_default(),
            Box::new(MockSensors::default()),
        );
        s.tick().unwrap();
        s.tick().unwrap();
        assert_eq!(s.cpu_history(), vec![42.0, 42.0]);
    }
}
