// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Live tests against the real OS. They spawn a throwaway child process and act on
//! it only, so they are safe to run in CI on Windows runners.

#![cfg(windows)]

use std::process::{Command, Stdio};
use std::time::Duration;

use tpt_monitor_core::{MonitorError, Pid, Priority, ProcessAction, SystemMonitor, execute_action};
use tpt_monitor_platform_windows::WindowsMonitor;

/// A child that sleeps for about a minute, so it outlives the test body.
fn spawn_sleeper() -> std::process::Child {
    Command::new("ping")
        .args(["-n", "60", "127.0.0.1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("ping should be available on Windows")
}

fn find(monitor: &mut WindowsMonitor, pid: u32) -> bool {
    monitor
        .sample_processes()
        .expect("process sampling should succeed")
        .iter()
        .any(|p| p.pid == Pid(pid))
}

#[test]
fn enumerates_the_child_process() {
    let mut child = spawn_sleeper();
    let mut monitor = WindowsMonitor::new();
    std::thread::sleep(Duration::from_millis(200));
    assert!(find(&mut monitor, child.id()));
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn system_metrics_look_sane() {
    let mut monitor = WindowsMonitor::new();
    let system = monitor.sample_system().unwrap();
    assert!(system.memory_total_bytes > 0);
    assert!(system.memory_used_bytes <= system.memory_total_bytes);
    assert_eq!(system.cpu_per_core_percent.len(), monitor.logical_cores());
    assert_eq!(system.net_up_bytes_per_sec, None);
}

#[test]
fn suspend_resume_priority_and_kill_on_child() {
    let mut child = spawn_sleeper();
    let pid = Pid(child.id());
    let mut monitor = WindowsMonitor::new();
    std::thread::sleep(Duration::from_millis(200));
    assert!(find(&mut monitor, child.id()));

    execute_action(&mut monitor, pid, ProcessAction::Suspend, false).unwrap();
    execute_action(&mut monitor, pid, ProcessAction::Resume, false).unwrap();
    execute_action(&mut monitor, pid, ProcessAction::SetPriority(Priority::BelowNormal), false)
        .unwrap();

    // Kill is destructive: refused without confirmation, child still alive.
    let refused = execute_action(&mut monitor, pid, ProcessAction::Kill, false).unwrap_err();
    assert!(matches!(refused, MonitorError::ConfirmationRequired(_)));
    assert!(child.try_wait().unwrap().is_none());

    execute_action(&mut monitor, pid, ProcessAction::Kill, true).unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
}

#[test]
fn acting_on_a_missing_pid_reports_not_found() {
    let mut monitor = WindowsMonitor::new();
    // PID 0x7FFFFFFC is far beyond any live PID on Windows.
    let err = monitor.apply_action(Pid(0x7FFF_FFFC), ProcessAction::Suspend).unwrap_err();
    assert!(matches!(
        err,
        MonitorError::ProcessNotFound(_) | MonitorError::PermissionDenied(_) | MonitorError::Os(_)
    ));
}
