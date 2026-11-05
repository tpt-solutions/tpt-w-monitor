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
}

impl Default for SuggestionConfig {
    fn default() -> Self {
        Self { window_samples: 30, cpu_threshold_percent: 15.0, sustain_ratio: 0.8 }
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
    background: bool,
}

/// Observes samples and produces suggestions.
#[derive(Debug)]
pub struct SuggestionEngine {
    config: SuggestionConfig,
    critical: CriticalList,
    dismissed: HashSet<String>,
    tracks: HashMap<Pid, Track>,
}

impl SuggestionEngine {
    /// Creates an engine with no observations and no dismissals.
    pub fn new(config: SuggestionConfig, critical: CriticalList) -> Self {
        Self { config, critical, dismissed: HashSet::new(), tracks: HashMap::new() }
    }

    /// Records one sample of process metrics. Tracks for exited processes are dropped.
    pub fn observe(&mut self, processes: &[ProcessMetrics]) {
        let live: HashSet<Pid> = processes.iter().map(|p| p.pid).collect();
        self.tracks.retain(|pid, _| live.contains(pid));

        for p in processes {
            let window = self.config.window_samples;
            let track = self.tracks.entry(p.pid).or_insert_with(|| Track {
                name: p.name.clone(),
                cpu: RingBuffer::new(window),
                background: false,
            });
            // A reused PID with a different executable starts a fresh history.
            if track.name != p.name {
                *track =
                    Track { name: p.name.clone(), cpu: RingBuffer::new(window), background: false };
            }
            track.cpu.push(p.cpu_percent);
            track.background = !p.has_visible_window;
        }
    }

    /// Stops suggesting for `process_name` ("don't suggest again"). Keyed by
    /// executable name so the setting survives PID reuse and restarts of the app.
    pub fn dismiss(&mut self, process_name: &str) {
        self.dismissed.insert(process_name.to_ascii_lowercase());
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
                if track.cpu.len() < cfg.window_samples || !track.background {
                    return None;
                }
                if self.critical.is_critical(&track.name) || self.is_dismissed(&track.name) {
                    return None;
                }

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
                    (
                        ProcessAction::SetPriority(Priority::BelowNormal),
                        "could run at lower priority",
                    )
                };

                Some(Suggestion {
                    pid: *pid,
                    process_name: track.name.clone(),
                    rationale: format!(
                        "{} is averaging {:.0}% CPU in the background and {verb}.",
                        track.name, avg
                    ),
                    recommended_action: action,
                    confidence: fraction,
                    impact_cpu_percent: avg,
                })
            })
            .collect();

        out.sort_by(|a, b| {
            b.impact_cpu_percent.total_cmp(&a.impact_cpu_percent).then(a.pid.cmp(&b.pid))
        });
        out
    }
}
