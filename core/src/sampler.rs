// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Sampling engine: pulls metrics from a backend, records short-window history,
//! and reports how long to wait before the next sample.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::action::{ProcessAction, execute_action};
use crate::error::Result;
use crate::history::RingBuffer;
use crate::model::{Pid, ProcessMetrics, SystemMetrics};
use crate::scheduler::SamplingScheduler;
use crate::traits::SystemMonitor;

/// One complete sample.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// System-wide metrics.
    pub system: SystemMetrics,
    /// Metrics for every process visible at this sample.
    pub processes: Vec<ProcessMetrics>,
}

/// Drives a backend and keeps history.
pub struct Sampler<M: SystemMonitor> {
    monitor: M,
    scheduler: SamplingScheduler,
    history_len: usize,
    system_cpu: RingBuffer<f32>,
    system_memory: RingBuffer<f32>,
    process_cpu: HashMap<Pid, RingBuffer<f32>>,
}

impl<M: SystemMonitor> Sampler<M> {
    /// Creates a sampler that keeps `history_len` samples per series.
    pub fn new(monitor: M, scheduler: SamplingScheduler, history_len: usize) -> Self {
        let history_len = history_len.max(1);
        Self {
            monitor,
            scheduler,
            history_len,
            system_cpu: RingBuffer::new(history_len),
            system_memory: RingBuffer::new(history_len),
            process_cpu: HashMap::new(),
        }
    }

    /// Takes one sample, records history, and forgets history for exited processes.
    pub fn sample(&mut self) -> Result<Snapshot> {
        let system = self.monitor.sample_system()?;
        let processes = self.monitor.sample_processes()?;

        self.system_cpu.push(system.cpu_total_percent);
        self.system_memory.push(system.memory_used_fraction());

        let live: HashSet<Pid> = processes.iter().map(|p| p.pid).collect();
        self.process_cpu.retain(|pid, _| live.contains(pid));
        for process in &processes {
            self.process_cpu
                .entry(process.pid)
                .or_insert_with(|| RingBuffer::new(self.history_len))
                .push(process.cpu_percent);
        }

        Ok(Snapshot { system, processes })
    }

    /// Applies a process action through the confirmation gate.
    pub fn apply_action(&mut self, pid: Pid, action: ProcessAction, confirmed: bool) -> Result<()> {
        execute_action(&mut self.monitor, pid, action, confirmed)
    }

    /// How long to wait before the next [`Self::sample`] call.
    pub fn next_interval(&self) -> Duration {
        self.scheduler.current_interval()
    }

    /// The scheduler, for updating the view state.
    pub fn scheduler_mut(&mut self) -> &mut SamplingScheduler {
        &mut self.scheduler
    }

    /// System CPU history, oldest first.
    pub fn system_cpu_history(&self) -> &RingBuffer<f32> {
        &self.system_cpu
    }

    /// System memory-used fraction history, oldest first.
    pub fn system_memory_history(&self) -> &RingBuffer<f32> {
        &self.system_memory
    }

    /// CPU history for one process, or `None` if it has not been seen (or has exited).
    pub fn process_cpu_history(&self, pid: Pid) -> Option<&RingBuffer<f32>> {
        self.process_cpu.get(&pid)
    }

    /// Shared access to the backend.
    pub fn monitor(&self) -> &M {
        &self.monitor
    }

    /// Mutable access to the backend (e.g. to change what a mock reports).
    pub fn monitor_mut(&mut self) -> &mut M {
        &mut self.monitor
    }
}
