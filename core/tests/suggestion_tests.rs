// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Suggestion engine against fixture process histories.

use tpt_monitor_core::SuggestionReason;
use tpt_monitor_core::{
    CriticalList, Pid, Priority, ProcessAction, ProcessMetrics, SuggestionConfig, SuggestionEngine,
};

fn cfg() -> SuggestionConfig {
    SuggestionConfig {
        window_samples: 4,
        cpu_threshold_percent: 15.0,
        sustain_ratio: 0.75,
        ..SuggestionConfig::default()
    }
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

#[test]
fn dismissed_names_are_listed_sorted_and_restorable() {
    let mut e = engine();
    e.dismiss("Zeta.exe");
    e.dismiss("alpha.exe");
    assert_eq!(e.dismissed_names(), vec!["alpha.exe", "zeta.exe"]);
    e.restore("ALPHA.exe");
    assert_eq!(e.dismissed_names(), vec!["zeta.exe"]);
}

#[test]
fn critical_names_are_listed_sorted_and_lowercase() {
    let list = CriticalList::new(["Services.exe", "csrss.exe"]);
    assert_eq!(list.names(), vec!["csrss.exe", "services.exe"]);
}

#[test]
fn foreground_process_is_suggested_while_the_user_is_idle() {
    let mut e = engine();
    e.set_user_idle(true);
    feed(&mut e, 60, "render.exe", &[80.0, 80.0, 80.0, 80.0], true);
    let s = e.suggestions();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].process_name, "render.exe");
}

#[test]
fn foreground_process_is_not_suggested_while_the_user_is_active() {
    let mut e = engine();
    assert!(!e.user_idle());
    feed(&mut e, 61, "render.exe", &[80.0, 80.0, 80.0, 80.0], true);
    assert!(e.suggestions().is_empty());
}

/// A process with the given CPU and disk read rate (bytes per second) for one sample.
fn io_sample(pid: u32, name: &str, cpu: f32, read_bps: u64, visible: bool) -> ProcessMetrics {
    let mut p = sample(pid, name, cpu, visible);
    p.disk_read_bytes_per_sec = Some(read_bps);
    p.disk_write_bytes_per_sec = Some(0);
    p
}

const MIB: u64 = 1024 * 1024;

#[test]
fn sustained_disk_io_in_the_background_is_suggested_as_a_suspend() {
    let mut e = engine();
    for _ in 0..4 {
        e.observe(&[io_sample(70, "backup.exe", 1.0, 20 * MIB, false)]);
    }
    let s = e.suggestions();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].reason, SuggestionReason::SustainedDisk);
    assert_eq!(s[0].recommended_action, ProcessAction::Suspend);
    assert!(s[0].rationale.contains("MiB/s"));
}

#[test]
fn brief_disk_burst_is_not_suggested() {
    let mut e = engine();
    let rates = [50 * MIB, MIB, 0, 0];
    for r in rates {
        e.observe(&[io_sample(71, "copy.exe", 1.0, r, false)]);
    }
    assert!(e.suggestions().is_empty());
}

#[test]
fn cpu_rule_wins_when_both_rules_fire_for_one_process() {
    let mut e = engine();
    for _ in 0..4 {
        e.observe(&[io_sample(72, "both.exe", 80.0, 20 * MIB, false)]);
    }
    let s = e.suggestions();
    assert_eq!(s.len(), 1, "one suggestion per process");
    assert_eq!(s[0].reason, SuggestionReason::SustainedCpu);
}

#[test]
fn critical_process_is_never_suggested_for_disk_either() {
    let mut e = engine();
    for _ in 0..4 {
        e.observe(&[io_sample(73, "csrss.exe", 1.0, 50 * MIB, false)]);
    }
    assert!(e.suggestions().is_empty());
}

#[test]
fn disk_suggestion_needs_the_user_away_or_the_app_in_the_background() {
    let mut e = engine();
    for _ in 0..4 {
        e.observe(&[io_sample(74, "editor.exe", 1.0, 20 * MIB, true)]);
    }
    assert!(e.suggestions().is_empty(), "foreground and user active");
    e.set_user_idle(true);
    assert_eq!(e.suggestions().len(), 1);
}

#[test]
fn user_protection_can_be_added_and_removed_but_builtins_cannot() {
    let mut list = CriticalList::windows_default();
    assert!(list.add("MyService.exe"));
    assert!(list.is_critical("C:\\tools\\myservice.exe"));
    assert_eq!(list.added_names(), vec!["myservice.exe"]);
    assert!(list.remove_added("myservice.exe"));
    assert!(!list.is_critical("myservice.exe"));

    assert!(!list.remove_added("csrss.exe"), "built-in names are not removable");
    assert!(list.is_critical("csrss.exe"));
    assert!(!list.add("csrss.exe"), "already protected by default");
}

#[test]
fn invalid_protection_names_are_refused() {
    let mut list = CriticalList::windows_default();
    for bad in ["", "   ", "a b.exe", "..\\evil", "name/with/slash", "x".repeat(65).as_str()] {
        assert!(!list.add(bad), "{bad:?} should be refused");
    }
    assert!(list.added_names().is_empty());
}

#[test]
fn engine_protection_stops_suggestions_for_that_process() {
    let mut e = engine();
    assert!(e.protect("Miner.exe"));
    feed(&mut e, 80, "miner.exe", &[60.0; 4], false);
    assert!(e.suggestions().is_empty());
    assert!(e.unprotect("miner.exe"));
    assert_eq!(e.suggestions().len(), 1);
}
