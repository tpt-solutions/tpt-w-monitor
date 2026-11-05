// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Smart suggestions: rule-based, suggest-only advice about background processes.
//!
//! The engine never applies anything. It observes samples, and when a background
//! process has been busy for a sustained window it proposes a reversible action.
//! Kill is never proposed automatically. Critical system processes are filtered
//! here in the engine, not only in the UI.

use std::collections::{HashMap, HashSet};

use crate::action::{Priority, ProcessAction};
use crate::history::RingBuffer;
use crate::model::{Pid, ProcessMetrics};

/// Which rule produced a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionReason {
    /// Sustained CPU use while in the background (or while the user is idle).
    SustainedCpu,
    /// Sustained disk I/O while in the background (or while the user is idle).
    SustainedDisk,
}

/// A proposed action with the reasoning behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// Process the suggestion is about (at the time of the observation).
    pub pid: Pid,
    /// Executable name.
    pub process_name: String,
    /// Human-readable explanation shown in the UI.
    pub rationale: String,
    /// The action the user is offered. Always reversible.
    pub recommended_action: ProcessAction,
    /// Fraction of the window in which the rule fired, 0.0..=1.0.
    pub confidence: f32,
    /// Average CPU over the window, used to sort the feed by impact.
    pub impact_cpu_percent: f32,
    /// The rule that fired.
    pub reason: SuggestionReason,
}

/// Thresholds for the sustained-CPU rule.
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionConfig {
    /// Number of samples in the rolling window. No suggestion is made until the
    /// window is full.
    pub window_samples: usize,
    /// CPU percent at or above which a sample counts as "busy".
    pub cpu_threshold_percent: f32,
    /// Fraction of the window that must be busy, 0.0..=1.0.
    pub sustain_ratio: f32,
    /// Read plus write rate, bytes per second, at or above which a sample counts as
    /// disk-busy. The default of 5 MiB/s is well above idle background noise.
    pub disk_threshold_bytes_per_sec: f32,
}

impl Default for SuggestionConfig {
    fn default() -> Self {
        Self {
            window_samples: 30,
            cpu_threshold_percent: 15.0,
            sustain_ratio: 0.8,
            disk_threshold_bytes_per_sec: 5.0 * 1024.0 * 1024.0,
        }
    }
}

/// Process names that must never be suggested for suspension or priority changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticalList {
    names: HashSet<String>,
}

impl CriticalList {
    /// Builds a list from names. Matching is case-insensitive.
    pub fn new<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self { names: names.into_iter().map(|n| n.as_ref().to_ascii_lowercase()).collect() }
    }

    /// Default Windows list: session, logon, shell and security processes.
    pub fn windows_default() -> Self {
        Self::new([
            "system",
            "system idle process",
            "registry",
            "smss.exe",
            "csrss.exe",
            "wininit.exe",
            "winlogon.exe",
            "services.exe",
            "lsass.exe",
            "svchost.exe",
            "dwm.exe",
            "explorer.exe",
            "sihost.exe",
            "fontdrvhost.exe",
            "tpt-monitor.exe",
        ])
    }

    /// Default Linux list: init, core services and display servers.
    pub fn linux_default() -> Self {
        Self::new([
            "systemd",
            "init",
            "kthreadd",
            "dbus-daemon",
            "systemd-journald",
            "systemd-logind",
            "Xorg",
            "Xwayland",
            "gnome-shell",
            "kwin_x11",
            "kwin_wayland",
            "tpt-monitor",
        ])
    }

    /// Every name on the list, lowercase and sorted, for display.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.names.iter().cloned().collect();
        names.sort();
        names
    }

    /// Whether `process` (a bare name or a full path) is on the list.
    pub fn is_critical(&self, process: &str) -> bool {
        let file = process.rsplit(['/', '\\']).next().unwrap_or(process);
        self.names.contains(&file.to_ascii_lowercase())
    }
}

#[derive(Debug)]
struct Track {
    name: String,
    cpu: RingBuffer<f32>,
    io: RingBuffer<f32>,
    background: bool,
}

impl Track {
    fn new(name: &str, window: usize) -> Self {
        Self {
            name: name.to_string(),
            cpu: RingBuffer::new(window),
            io: RingBuffer::new(window),
            background: false,
        }
    }
}

/// Observes samples and produces suggestions.
#[derive(Debug)]
pub struct SuggestionEngine {
    config: SuggestionConfig,
    critical: CriticalList,
    dismissed: HashSet<String>,
    tracks: HashMap<Pid, Track>,
    user_idle: bool,
}

impl SuggestionEngine {
    /// Creates an engine with no observations and no dismissals.
    pub fn new(config: SuggestionConfig, critical: CriticalList) -> Self {
        Self {
            config,
            critical,
            dismissed: HashSet::new(),
            tracks: HashMap::new(),
            user_idle: false,
        }
    }

    /// Records one sample of process metrics. Tracks for exited processes are dropped.
    pub fn observe(&mut self, processes: &[ProcessMetrics]) {
        let live: HashSet<Pid> = processes.iter().map(|p| p.pid).collect();
        self.tracks.retain(|pid, _| live.contains(pid));

        for p in processes {
            let window = self.config.window_samples;
            let track = self.tracks.entry(p.pid).or_insert_with(|| Track::new(&p.name, window));
            // A reused PID with a different executable starts a fresh history.
            if track.name != p.name {
                *track = Track::new(&p.name, window);
            }
            track.cpu.push(p.cpu_percent);
            let io = match (p.disk_read_bytes_per_sec, p.disk_write_bytes_per_sec) {
                (None, None) => 0.0,
                (r, w) => (r.unwrap_or(0) + w.unwrap_or(0)) as f32,
            };
            track.io.push(io);
            track.background = !p.has_visible_window;
        }
    }

    /// Records whether the user has been idle long enough. While idle, a sustained busy
    /// process is suggested even if it has a visible window.
    pub fn set_user_idle(&mut self, idle: bool) {
        self.user_idle = idle;
    }

    /// Whether the user is currently treated as idle.
    pub fn user_idle(&self) -> bool {
        self.user_idle
    }

    /// Stops suggesting for `process_name` ("don't suggest again"). Keyed by
    /// executable name so the setting survives PID reuse and restarts of the app.
    pub fn dismiss(&mut self, process_name: &str) {
        self.dismissed.insert(process_name.to_ascii_lowercase());
    }

    /// Executable names whose suggestions are dismissed, sorted.
    pub fn dismissed_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.dismissed.iter().cloned().collect();
        names.sort();
        names
    }

    /// Re-enables suggestions for `process_name`.
    pub fn restore(&mut self, process_name: &str) {
        self.dismissed.remove(&process_name.to_ascii_lowercase());
    }

    /// Whether suggestions for `process_name` have been dismissed.
    pub fn is_dismissed(&self, process_name: &str) -> bool {
        self.dismissed.contains(&process_name.to_ascii_lowercase())
    }

    /// The critical list this engine enforces.
    pub fn critical_list(&self) -> &CriticalList {
        &self.critical
    }

    /// Current suggestions, highest impact first.
    ///
    /// A process qualifies when its window is full, it is currently in the
    /// background, it is not critical or dismissed, and at least
    /// `sustain_ratio` of its samples were at or above the CPU threshold.
    /// Processes busier than twice the threshold get a suspend suggestion; the
    /// rest get a lower-priority suggestion.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let cfg = &self.config;
        let mut out: Vec<Suggestion> = self
            .tracks
            .iter()
            .filter_map(|(pid, track)| {
                // Qualifies when the process is in the background, or the user is idle.
                let eligible = track.background || self.user_idle;
                if track.cpu.len() < cfg.window_samples || !eligible {
                    return None;
                }
                if self.critical.is_critical(&track.name) || self.is_dismissed(&track.name) {
                    return None;
                }
                // CPU is the stronger signal, so it wins when both rules fire.
                self.cpu_suggestion(*pid, track).or_else(|| self.disk_suggestion(*pid, track))
            })
            .collect();

        out.sort_by(|a, b| {
            b.impact_cpu_percent.total_cmp(&a.impact_cpu_percent).then(a.pid.cmp(&b.pid))
        });
        out
    }
}

impl SuggestionEngine {
    /// Sustained CPU rule for one process, if it fires.
    fn cpu_suggestion(&self, pid: Pid, track: &Track) -> Option<Suggestion> {
        let cfg = &self.config;
        let total = track.cpu.len() as f32;
        let busy = track.cpu.iter().filter(|&&c| c >= cfg.cpu_threshold_percent).count();
        let fraction = busy as f32 / total;
        if fraction < cfg.sustain_ratio {
            return None;
        }
        let avg = track.cpu.iter().sum::<f32>() / total;
        let (action, verb) = if avg >= cfg.cpu_threshold_percent * 2.0 {
            (ProcessAction::Suspend, "can be suspended")
        } else {
            (ProcessAction::SetPriority(Priority::BelowNormal), "could run at lower priority")
        };
        Some(Suggestion {
            pid,
            process_name: track.name.clone(),
            rationale: format!(
                "{} is averaging {:.0}% CPU in the background and {verb}.",
                track.name, avg
            ),
            recommended_action: action,
            confidence: fraction,
            impact_cpu_percent: avg,
            reason: SuggestionReason::SustainedCpu,
        })
    }

    /// Sustained disk rule for one process, if it fires. Suspending stops the I/O, which
    /// is the reversible action that addresses it.
    fn disk_suggestion(&self, pid: Pid, track: &Track) -> Option<Suggestion> {
        let cfg = &self.config;
        let total = track.io.len() as f32;
        let busy = track.io.iter().filter(|&&b| b >= cfg.disk_threshold_bytes_per_sec).count();
        let fraction = busy as f32 / total;
        if fraction < cfg.sustain_ratio {
            return None;
        }
        let avg_rate = track.io.iter().sum::<f32>() / total;
        let avg_cpu = track.cpu.iter().sum::<f32>() / total;
        let mib = avg_rate / (1024.0 * 1024.0);
        Some(Suggestion {
            pid,
            process_name: track.name.clone(),
            rationale: format!(
                "{} is averaging {:.1} MiB/s of disk I/O in the background and can be suspended.",
                track.name, mib
            ),
            recommended_action: ProcessAction::Suspend,
            confidence: fraction,
            impact_cpu_percent: avg_cpu,
            reason: SuggestionReason::SustainedDisk,
        })
    }
}
