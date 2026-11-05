// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! IPC boundary rules, tested against the mock backend.

use std::time::Duration;

use tpt_monitor_core::mock::{MockMonitor, MockSensors};
use tpt_monitor_core::{
    CriticalList, MonitorError, Pid, Priority, ProcessAction, ProcessMetrics, ViewState,
};
use tpt_monitor_tauri::state::{MonitorState, USER_IDLE_SECS};

fn proc(pid: u32, name: &str, cpu: f32, visible: bool) -> ProcessMetrics {
    let mut p = ProcessMetrics::new(Pid(pid), name);
    p.cpu_percent = cpu;
    p.has_visible_window = visible;
    p
}

fn state_with(processes: Vec<ProcessMetrics>) -> MonitorState<MockMonitor> {
    let mut mock = MockMonitor::new();
    mock.processes = processes;
    MonitorState::new(mock, CriticalList::windows_default(), Box::new(MockSensors::default()))
}

fn state_with_critical_and_app() -> MonitorState<MockMonitor> {
    state_with(vec![proc(4, "csrss.exe", 1.0, false), proc(100, "chrome.exe", 5.0, true)])
}

#[test]
fn unknown_pid_is_refused_before_any_tick() {
    let mut s = state_with_critical_and_app();
    let err = s.apply_action(Pid(100), ProcessAction::Suspend, false).unwrap_err();
    assert_eq!(err, MonitorError::ProcessNotFound(Pid(100)));
}

#[test]
fn pid_outside_latest_snapshot_is_refused() {
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    let err = s.apply_action(Pid(9999), ProcessAction::Suspend, false).unwrap_err();
    assert_eq!(err, MonitorError::ProcessNotFound(Pid(9999)));
}

#[test]
fn kill_requires_confirmation() {
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    let err = s.apply_action(Pid(100), ProcessAction::Kill, false).unwrap_err();
    assert_eq!(err, MonitorError::ConfirmationRequired(ProcessAction::Kill));
}

#[test]
fn confirmed_kill_on_normal_process_runs() {
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    s.apply_action(Pid(100), ProcessAction::Kill, true).unwrap();
}

#[test]
fn critical_process_cannot_be_killed_even_when_confirmed() {
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    let err = s.apply_action(Pid(4), ProcessAction::Kill, true).unwrap_err();
    assert!(matches!(err, MonitorError::InvalidAction(_)));
}

#[test]
fn critical_process_cannot_be_suspended() {
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    let err = s.apply_action(Pid(4), ProcessAction::Suspend, false).unwrap_err();
    assert!(matches!(err, MonitorError::InvalidAction(_)));
}

#[test]
fn priority_change_is_allowed_on_critical_process() {
    // Lowering priority is not disruptive in the way suspend or kill is.
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    s.apply_action(Pid(4), ProcessAction::SetPriority(Priority::BelowNormal), false).unwrap();
}

#[test]
fn zero_affinity_is_rejected_at_the_boundary() {
    let mut s = state_with_critical_and_app();
    s.tick().unwrap();
    let err = s.apply_action(Pid(100), ProcessAction::SetAffinity(0), true).unwrap_err();
    assert!(matches!(err, MonitorError::InvalidAction(_)));
}

#[test]
fn tick_publishes_suggestions_after_a_full_window() {
    let mut s = state_with(vec![proc(200, "miner.exe", 60.0, false)]);
    for _ in 0..29 {
        assert!(s.tick().unwrap().suggestions.is_empty());
    }
    let frame = s.tick().unwrap();
    assert_eq!(frame.suggestions.len(), 1);
    assert_eq!(frame.suggestions[0].process_name, "miner.exe");
}

#[test]
fn dismissed_suggestion_disappears_from_next_frame() {
    let mut s = state_with(vec![proc(200, "miner.exe", 60.0, false)]);
    for _ in 0..30 {
        s.tick().unwrap();
    }
    s.dismiss_suggestion("miner.exe");
    assert!(s.tick().unwrap().suggestions.is_empty());
    s.restore_suggestion("miner.exe");
    assert_eq!(s.tick().unwrap().suggestions.len(), 1);
}

#[test]
fn view_state_changes_the_sampling_interval() {
    let mut s = state_with(vec![]);
    s.set_view_state(ViewState {
        dashboard_visible: true,
        dashboard_focused: true,
        open_overlays: 0,
    });
    assert_eq!(s.next_interval(), Duration::from_secs(1));
    s.set_view_state(ViewState::default());
    assert_eq!(s.next_interval(), Duration::from_secs(8));
}

#[test]
fn busy_visible_process_is_suggested_only_after_the_user_goes_idle() {
    let mut s = state_with(vec![proc(300, "encoder.exe", 80.0, true)]);
    s.monitor_mut().idle_secs = Some(10);
    for _ in 0..30 {
        assert!(s.tick().unwrap().suggestions.is_empty(), "active user: no suggestion");
    }
    s.monitor_mut().idle_secs = Some(USER_IDLE_SECS);
    assert_eq!(s.tick().unwrap().suggestions.len(), 1);
}

#[test]
fn user_protected_process_is_refused_for_kill_even_when_confirmed() {
    let mut s = state_with(vec![proc(400, "mine.exe", 1.0, true)]);
    s.tick().unwrap();
    assert!(s.protect_process("mine.exe"));
    let err = s.apply_action(Pid(400), ProcessAction::Kill, true).unwrap_err();
    assert!(matches!(err, MonitorError::InvalidAction(_)));
    assert!(s.unprotect_process("mine.exe"));
    assert!(s.apply_action(Pid(400), ProcessAction::Kill, true).is_ok());
}
