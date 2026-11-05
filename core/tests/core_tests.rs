// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Core behaviour against the mock backend.

use std::time::Duration;

use tpt_monitor_core::mock::{MockMonitor, MockSensors};
use tpt_monitor_core::{
    FanReading, MonitorError, Pid, Priority, ProcessAction, ProcessMetrics, RingBuffer, Sampler,
    SamplingScheduler, SensorProvider, SystemMetrics, TemperatureReading, ViewState,
    execute_action,
};

fn proc(pid: u32, name: &str, cpu: f32) -> ProcessMetrics {
    let mut p = ProcessMetrics::new(Pid(pid), name);
    p.cpu_percent = cpu;
    p
}

fn mock_with(processes: Vec<ProcessMetrics>) -> MockMonitor {
    let mut m = MockMonitor::new();
    m.processes = processes;
    m
}

// --- RingBuffer ------------------------------------------------------------

#[test]
fn ring_buffer_evicts_oldest_and_keeps_order() {
    let mut rb = RingBuffer::new(3);
    for v in 1..=5 {
        rb.push(v);
    }
    assert_eq!(rb.len(), 3);
    assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    assert_eq!(rb.latest(), Some(&5));
}

#[test]
fn ring_buffer_zero_capacity_is_raised_to_one() {
    let mut rb = RingBuffer::new(0);
    assert_eq!(rb.capacity(), 1);
    rb.push('a');
    rb.push('b');
    assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec!['b']);
}

#[test]
fn ring_buffer_clear_empties_it() {
    let mut rb = RingBuffer::new(2);
    rb.push(1);
    rb.clear();
    assert!(rb.is_empty());
    assert_eq!(rb.latest(), None);
}

// --- Memory model ------------------------------------------------------------

#[test]
fn memory_fraction_is_zero_when_total_unknown() {
    let s = SystemMetrics::default();
    assert_eq!(s.memory_used_fraction(), 0.0);
}

#[test]
fn memory_fraction_is_clamped_to_one() {
    let s = SystemMetrics {
        memory_total_bytes: 100,
        memory_used_bytes: 250,
        ..SystemMetrics::default()
    };
    assert_eq!(s.memory_used_fraction(), 1.0);
}

// --- Scheduler ---------------------------------------------------------------

#[test]
fn scheduler_full_speed_when_dashboard_focused() {
    let mut s = SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(60));
    s.set_view_state(ViewState {
        dashboard_visible: true,
        dashboard_focused: true,
        open_overlays: 0,
    });
    assert_eq!(s.current_interval(), Duration::from_secs(1));
}

#[test]
fn scheduler_slows_down_in_steps() {
    let mut s = SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(60));

    s.set_view_state(ViewState {
        dashboard_visible: true,
        dashboard_focused: false,
        open_overlays: 0,
    });
    assert_eq!(s.current_interval(), Duration::from_secs(4));

    s.set_view_state(ViewState {
        dashboard_visible: false,
        dashboard_focused: false,
        open_overlays: 0,
    });
    assert_eq!(s.current_interval(), Duration::from_secs(8));

    s.set_view_state(ViewState {
        dashboard_visible: false,
        dashboard_focused: false,
        open_overlays: 2,
    });
    assert_eq!(s.current_interval(), Duration::from_secs(2));
}

#[test]
fn scheduler_caps_at_max() {
    // Default view (hidden, no overlays) would be 8x = 40s; capped at 10s.
    let s = SamplingScheduler::new(Duration::from_secs(5), Duration::from_secs(10));
    assert_eq!(s.current_interval(), Duration::from_secs(10));
}

#[test]
fn scheduler_raises_inverted_max_to_base() {
    // max < base is raised to base, so the interval never drops below base.
    let s = SamplingScheduler::new(Duration::from_secs(5), Duration::from_secs(1));
    assert_eq!(s.current_interval(), Duration::from_secs(5));
}

// --- Sampler -----------------------------------------------------------------

#[test]
fn sampler_records_system_and_process_history() {
    let mut monitor = mock_with(vec![proc(1, "a.exe", 10.0)]);
    monitor.system.cpu_total_percent = 30.0;
    let mut sampler = Sampler::new(
        monitor,
        SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(8)),
        10,
    );

    sampler.sample().unwrap();
    sampler.sample().unwrap();

    assert_eq!(sampler.system_cpu_history().len(), 2);
    assert_eq!(sampler.system_cpu_history().latest(), Some(&30.0));
    assert_eq!(sampler.process_cpu_history(Pid(1)).unwrap().len(), 2);
}

#[test]
fn sampler_forgets_history_for_exited_processes() {
    let monitor = mock_with(vec![proc(1, "a.exe", 1.0), proc(2, "b.exe", 2.0)]);
    let mut sampler = Sampler::new(
        monitor,
        SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(8)),
        10,
    );
    sampler.sample().unwrap();
    assert!(sampler.process_cpu_history(Pid(2)).is_some());

    // Process 2 exits between samples.
    sampler.monitor_mut().processes.retain(|p| p.pid != Pid(2));
    sampler.sample().unwrap();

    assert!(sampler.process_cpu_history(Pid(2)).is_none());
    assert_eq!(sampler.process_cpu_history(Pid(1)).unwrap().len(), 2);
}

#[test]
fn sampler_history_is_bounded() {
    let monitor = mock_with(vec![proc(1, "a.exe", 1.0)]);
    let mut sampler = Sampler::new(
        monitor,
        SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(8)),
        3,
    );
    for _ in 0..10 {
        sampler.sample().unwrap();
    }
    assert_eq!(sampler.system_cpu_history().len(), 3);
    assert_eq!(sampler.process_cpu_history(Pid(1)).unwrap().len(), 3);
}

// --- Action gate ---------------------------------------------------------------

#[test]
fn kill_without_confirmation_never_reaches_backend() {
    let mut monitor = mock_with(vec![proc(7, "victim.exe", 0.0)]);
    let err = execute_action(&mut monitor, Pid(7), ProcessAction::Kill, false).unwrap_err();
    assert_eq!(err, MonitorError::ConfirmationRequired(ProcessAction::Kill));
    assert!(monitor.applied.is_empty());
    assert_eq!(monitor.processes.len(), 1);
}

#[test]
fn kill_with_confirmation_runs_and_removes_process() {
    let mut monitor = mock_with(vec![proc(7, "victim.exe", 0.0)]);
    execute_action(&mut monitor, Pid(7), ProcessAction::Kill, true).unwrap();
    assert_eq!(monitor.applied, vec![(Pid(7), ProcessAction::Kill)]);
    assert!(monitor.processes.is_empty());
}

#[test]
fn reversible_actions_do_not_need_confirmation() {
    let mut monitor = mock_with(vec![proc(3, "bg.exe", 0.0)]);
    execute_action(&mut monitor, Pid(3), ProcessAction::Suspend, false).unwrap();
    execute_action(&mut monitor, Pid(3), ProcessAction::Resume, false).unwrap();
    execute_action(&mut monitor, Pid(3), ProcessAction::SetPriority(Priority::BelowNormal), false)
        .unwrap();
    assert_eq!(monitor.applied.len(), 3);
}

#[test]
fn zero_affinity_mask_is_rejected_before_backend() {
    let mut monitor = mock_with(vec![proc(3, "bg.exe", 0.0)]);
    let err =
        execute_action(&mut monitor, Pid(3), ProcessAction::SetAffinity(0), true).unwrap_err();
    assert!(matches!(err, MonitorError::InvalidAction(_)));
    assert!(monitor.applied.is_empty());
}

#[test]
fn unknown_pid_reports_not_found() {
    let mut monitor = mock_with(vec![]);
    let err = execute_action(&mut monitor, Pid(99), ProcessAction::Suspend, false).unwrap_err();
    assert_eq!(err, MonitorError::ProcessNotFound(Pid(99)));
}

#[test]
fn backend_errors_propagate() {
    let mut monitor = mock_with(vec![proc(5, "x.exe", 0.0)]);
    monitor.fail_actions_with = Some(MonitorError::PermissionDenied(Pid(5)));
    let err = execute_action(&mut monitor, Pid(5), ProcessAction::Suspend, false).unwrap_err();
    assert_eq!(err, MonitorError::PermissionDenied(Pid(5)));
}

#[test]
fn sampler_apply_action_goes_through_gate() {
    let monitor = mock_with(vec![proc(8, "y.exe", 0.0)]);
    let mut sampler = Sampler::new(
        monitor,
        SamplingScheduler::new(Duration::from_secs(1), Duration::from_secs(8)),
        4,
    );
    assert!(sampler.apply_action(Pid(8), ProcessAction::Kill, false).is_err());
    assert!(sampler.apply_action(Pid(8), ProcessAction::Kill, true).is_ok());
}

// --- Sensors -------------------------------------------------------------------

#[test]
fn unavailable_sensors_return_none_not_zero() {
    let sensors = MockSensors::default();
    assert_eq!(sensors.temperatures(), None);
    assert_eq!(sensors.fans(), None);
}

#[test]
fn supported_but_empty_sensors_return_empty_vec() {
    let sensors = MockSensors { temperatures: Some(vec![]), fans: Some(vec![]) };
    assert_eq!(sensors.temperatures(), Some(vec![]));
    assert_eq!(sensors.fans(), Some(vec![]));
}

#[test]
fn sensor_readings_round_trip() {
    let sensors = MockSensors {
        temperatures: Some(vec![TemperatureReading { label: "CPU Package".into(), celsius: 61.5 }]),
        fans: Some(vec![FanReading { label: "CPU Fan".into(), rpm: 1200 }]),
    };
    assert_eq!(sensors.temperatures().unwrap()[0].celsius, 61.5);
    assert_eq!(sensors.fans().unwrap()[0].rpm, 1200);
}

#[test]
fn monitor_availability_flag_is_reported() {
    let mut m = MockMonitor::new();
    assert!(tpt_monitor_core::SystemMonitor::is_available(&m));
    m.available = false;
    assert!(!tpt_monitor_core::SystemMonitor::is_available(&m));
}
