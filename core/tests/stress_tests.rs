// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Scale checks: thousands of processes through the sampler and the suggestion engine.
//! They assert correctness at size. Timings are printed for information only, because
//! wall-clock assertions are unreliable on shared CI runners.

use std::time::{Duration, Instant};

use tpt_monitor_core::mock::MockMonitor;
use tpt_monitor_core::{
    CriticalList, Pid, ProcessMetrics, Sampler, SamplingScheduler, SuggestionConfig,
    SuggestionEngine,
};

const PROCESSES: u32 = 3_000;
const SAMPLES: usize = 40;

/// `PROCESSES` processes; every tenth one is a busy background job, the rest are idle.
fn population(sample: usize) -> Vec<ProcessMetrics> {
    (0..PROCESSES)
        .map(|i| {
            let mut p = ProcessMetrics::new(Pid(i + 1), format!("app{i}.exe"));
            p.has_visible_window = i % 10 != 0;
            p.cpu_percent = if i % 10 == 0 { 60.0 } else { (sample % 3) as f32 };
            p
        })
        .collect()
}

#[test]
fn engine_suggests_exactly_the_busy_background_jobs_at_scale() {
    let mut engine =
        SuggestionEngine::new(SuggestionConfig::default(), CriticalList::windows_default());
    let started = Instant::now();
    for sample in 0..SAMPLES {
        engine.observe(&population(sample));
    }
    let suggestions = engine.suggestions();
    eprintln!("engine: {PROCESSES} processes x {SAMPLES} samples in {:?}", started.elapsed());

    // Only the busy jobs with no window qualify: every tenth process, and those have
    // visible = false, so all 300 of them should be suggested.
    assert_eq!(suggestions.len(), (PROCESSES / 10) as usize);
    assert!(suggestions.iter().all(|s| s.pid.0 % 10 == 1));
    // Highest impact first, ties broken by PID.
    assert!(suggestions.windows(2).all(|w| {
        w[0].impact_cpu_percent > w[1].impact_cpu_percent
            || (w[0].impact_cpu_percent == w[1].impact_cpu_percent && w[0].pid < w[1].pid)
    }));
}

#[test]
fn sampler_keeps_history_bounded_and_forgets_exited_processes_at_scale() {
    let mut monitor = MockMonitor::new();
    monitor.processes = population(0);
    let mut sampler = Sampler::new(
        monitor,
        SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(8)),
        60,
    );
    let started = Instant::now();
    for sample in 0..200 {
        sampler.monitor_mut().processes = population(sample);
        sampler.sample().unwrap();
    }
    eprintln!("sampler: {PROCESSES} processes x 200 samples in {:?}", started.elapsed());

    // History is capped at the configured length, for the system and for each process.
    assert_eq!(sampler.system_cpu_history().len(), 60);
    assert_eq!(sampler.process_cpu_history(Pid(1)).unwrap().len(), 60);

    // Half the processes exit: their history is dropped, the rest keep theirs.
    sampler.monitor_mut().processes.retain(|p| p.pid.0 % 2 == 0);
    sampler.sample().unwrap();
    assert!(sampler.process_cpu_history(Pid(1)).is_none());
    assert!(sampler.process_cpu_history(Pid(2)).is_some());
}
