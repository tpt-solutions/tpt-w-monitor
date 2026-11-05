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

/// Whether this process is elevated. `net session` succeeds only for administrators.
fn is_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn tcp_traffic_is_attributed_to_this_process() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;

    if !is_elevated() {
        eprintln!("skipped: per-connection byte counters need an elevated test run");
        return;
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Wait until the monitor has enabled counters, then send 1 MiB.
        go_rx.recv().expect("go signal");
        stream.write_all(&vec![7u8; 1 << 20]).expect("write");
        // Hold the connection open until the client has finished reading.
        let mut end = [0u8; 1];
        let _ = stream.read(&mut end);
    });

    let mut client = TcpStream::connect(addr).expect("connect");
    let mut monitor = WindowsMonitor::new();
    // Baseline sample: this also switches on byte counters for our connections.
    monitor.sample_processes().expect("baseline sample");
    go_tx.send(()).expect("signal server");

    let mut buf = vec![0u8; 1 << 20];
    client.read_exact(&mut buf).expect("read payload");
    std::thread::sleep(Duration::from_millis(300));

    let processes = monitor.sample_processes().expect("second sample");
    let me = processes
        .iter()
        .find(|p| p.pid == Pid(std::process::id()))
        .expect("this process is listed");
    let down = me.net_down_bytes_per_sec;

    drop(client);
    server.join().expect("server thread");

    assert!(down.unwrap_or(0) > 0, "expected download traffic for this process, got {down:?}");
}

#[test]
fn network_fields_are_reported_even_without_counters() {
    // Works unelevated: the connection table is readable, and without counters the
    // process reads zero (not an error). Elevated runs cover the counted path above.
    let mut monitor = WindowsMonitor::new();
    monitor.sample_processes().expect("first sample");
    let processes = monitor.sample_processes().expect("second sample");
    let me = processes
        .iter()
        .find(|p| p.pid == Pid(std::process::id()))
        .expect("this process is listed");
    assert!(me.net_up_bytes_per_sec.is_some());
    assert!(me.net_down_bytes_per_sec.is_some());
}

#[test]
fn gpu_counters_report_when_the_machine_has_them() {
    // Machines without a WDDM GPU driver have no GPU counters. Then every process reads
    // None and the test says so instead of failing.
    let mut monitor = WindowsMonitor::new();
    monitor.sample_processes().expect("first sample");
    let processes = monitor.sample_processes().expect("second sample");
    let with_gpu = processes.iter().filter(|p| p.gpu_engine_percent.is_some()).count();
    if with_gpu == 0 {
        eprintln!("no GPU counters on this machine; GPU fields stay unavailable");
        assert!(processes.iter().all(|p| p.vram_bytes.is_none()));
        return;
    }
    assert!(
        processes.iter().filter_map(|p| p.gpu_engine_percent).all(|v| (0.0..=100.0).contains(&v))
    );
    eprintln!("GPU counters reported for {with_gpu} processes");
}

#[test]
fn user_idle_time_is_reported() {
    let monitor = WindowsMonitor::new();
    let idle = monitor.user_idle_secs().expect("GetLastInputInfo should succeed");
    // Input can be older than any reasonable session, but a wrapped counter would show
    // as a value near 49 days. Anything under a day is plausible.
    assert!(idle < 86_400, "implausible idle time: {idle}s");
}

#[test]
fn elevation_check_agrees_with_net_session() {
    // `net session` succeeds only for administrators, so it is an independent oracle for
    // the token check.
    assert_eq!(tpt_monitor_platform_windows::is_elevated(), is_elevated());
}

#[test]
fn whole_machine_network_totals_appear_from_the_second_sample() {
    let mut monitor = WindowsMonitor::new();
    let first = monitor.sample_system().expect("first system sample");
    assert_eq!(first.net_down_bytes_per_sec, None, "no baseline yet");
    std::thread::sleep(Duration::from_millis(200));
    let second = monitor.sample_system().expect("second system sample");
    assert!(second.net_up_bytes_per_sec.is_some());
    assert!(second.net_down_bytes_per_sec.is_some());
}
