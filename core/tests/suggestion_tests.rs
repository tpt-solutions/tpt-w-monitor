// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Suggestion engine against fixture process histories.

use tpt_monitor_core::{
    CriticalList, Pid, Priority, ProcessAction, ProcessMetrics, SuggestionConfig, SuggestionEngine,
};

fn cfg() -> SuggestionConfig {
    SuggestionConfig { window_samples: 4, cpu_threshold_percent: 15.0, sustain_ratio: 0.75 }
}

fn engine() -> SuggestionEngine {
    SuggestionEngine::new(cfg(), CriticalList::new(["csrss.exe"]))
}

fn sample(pid: u32, name: &str, cpu: f32, visible: bool) -> ProcessMetrics {
    let mut p = ProcessMetrics::new(Pid(pid), name);
    p.cpu_percent = cpu;
    p.has_visible_window = visible;
    p
}

/// Feeds one CPU series for a single background process.
fn feed(e: &mut SuggestionEngine, pid: u32, name: &str, series: &[f32], visible: bool) {
    for &cpu in series {
        e.observe(&[sample(pid, name, cpu, visible)]);
    }
}

#[test]
fn no_suggestion_until_window_is_full() {
    let mut e = engine();
    feed(&mut e, 10, "miner.exe", &[50.0, 50.0, 50.0], false);
    assert!(e.suggestions().is_empty());
}

#[test]
fn sustained_heavy_background_process_gets_suspend() {
    let mut e = engine();
    feed(&mut e, 10, "miner.exe", &[60.0, 70.0, 65.0, 80.0], false);
    let s = e.suggestions();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].process_name, "miner.exe");
    assert_eq!(s[0].recommended_action, ProcessAction::Suspend);
    assert_eq!(s[0].confidence, 1.0);
    assert!(s[0].rationale.contains("miner.exe"));
}

#[test]
fn moderate_background_process_gets_lower_priority_not_suspend() {
    let mut e = engine();
    feed(&mut e, 11, "indexer.exe", &[20.0, 22.0, 18.0, 25.0], false);
    let s = e.suggestions();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].recommended_action, ProcessAction::SetPriority(Priority::BelowNormal));
}

#[test]
fn brief_spike_is_not_sustained() {
    let mut e = engine();
    // Only one of four samples is busy: below the 0.75 sustain ratio.
    feed(&mut e, 12, "spiky.exe", &[90.0, 2.0, 3.0, 1.0], false);
    assert!(e.suggestions().is_empty());
}

#[test]
fn foreground_process_is_never_suggested() {
    let mut e = engine();
    feed(&mut e, 13, "game.exe", &[80.0, 80.0, 80.0, 80.0], true);
    assert!(e.suggestions().is_empty());
}

#[test]
fn critical_process_never_triggers_even_when_busy() {
    let mut e = engine();
    feed(&mut e, 14, "csrss.exe", &[90.0, 90.0, 90.0, 90.0], false);
    feed(&mut e, 15, "C:\\Windows\\System32\\CSRSS.EXE", &[90.0; 4], false);
    assert!(e.suggestions().is_empty());
}

#[test]
fn critical_list_matches_paths_and_case() {
    let list = CriticalList::windows_default();
    assert!(list.is_critical("C:\\Windows\\System32\\lsass.exe"));
    assert!(list.is_critical("SERVICES.EXE"));
    assert!(!list.is_critical("chrome.exe"));

    let linux = CriticalList::linux_default();
    assert!(linux.is_critical("/usr/lib/systemd/systemd"));
    assert!(!linux.is_critical("firefox"));
}

#[test]
fn dismissed_process_is_not_suggested_again() {
    let mut e = engine();
    feed(&mut e, 10, "miner.exe", &[60.0; 4], false);
    assert_eq!(e.suggestions().len(), 1);

    e.dismiss("MINER.exe");
    assert!(e.is_dismissed("miner.exe"));
    assert!(e.suggestions().is_empty());

    e.restore("miner.exe");
    assert_eq!(e.suggestions().len(), 1);
}

#[test]
fn pid_reuse_with_new_name_starts_fresh_history() {
    let mut e = engine();
    feed(&mut e, 20, "old.exe", &[60.0; 4], false);
    // PID 20 is now a different program with a short, quiet history.
    feed(&mut e, 20, "new.exe", &[1.0; 4], false);
    assert!(e.suggestions().is_empty());
}

#[test]
fn exited_process_is_forgotten() {
    let mut e = engine();
    feed(&mut e, 30, "gone.exe", &[60.0; 4], false);
    e.observe(&[]);
    // It reappears with a short history: no suggestion from stale data.
    e.observe(&[sample(30, "gone.exe", 60.0, false)]);
    assert!(e.suggestions().is_empty());
}

#[test]
fn suggestions_sorted_by_impact() {
    let mut e = engine();
    // observe() takes a full snapshot, so both processes must appear in each sample.
    for _ in 0..4 {
        e.observe(&[sample(40, "small.exe", 20.0, false), sample(41, "big.exe", 90.0, false)]);
    }
    let names: Vec<_> = e.suggestions().into_iter().map(|s| s.process_name).collect();
    assert_eq!(names, vec!["big.exe", "small.exe"]);
}

#[test]
fn kill_is_never_recommended() {
    let mut e = engine();
    feed(&mut e, 50, "hog.exe", &[100.0; 4], false);
    for s in e.suggestions() {
        assert_ne!(s.recommended_action, ProcessAction::Kill);
        assert!(!s.recommended_action.requires_confirmation());
    }
}
