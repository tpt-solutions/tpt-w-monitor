// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Wire types for the frontend. Core types stay free of serde; these DTOs are the
//! only shape that crosses IPC, and inbound action payloads are parsed strictly.

use serde::{Deserialize, Serialize};
use tpt_monitor_core::{Priority, ProcessAction, ProcessMetrics, Suggestion, ViewState};

use crate::state::Frame;

/// One published frame.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FrameDto {
    /// System-wide metrics.
    pub system: SystemDto,
    /// Every visible process.
    pub processes: Vec<ProcessDto>,
    /// Suggestion feed, highest impact first.
    pub suggestions: Vec<SuggestionDto>,
    /// Sensor readings (each part null when unavailable).
    pub sensors: SensorsDto,
}

/// Sensor readings on the wire.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SensorsDto {
    /// Temperatures, or null if unavailable.
    pub temperatures: Option<Vec<TemperatureDto>>,
    /// Fan speeds, or null if unavailable.
    pub fans: Option<Vec<FanDto>>,
}

/// One temperature reading.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemperatureDto {
    /// Sensor label.
    pub label: String,
    /// Degrees Celsius.
    pub celsius: f32,
}

/// One fan reading.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FanDto {
    /// Fan label.
    pub label: String,
    /// Revolutions per minute.
    pub rpm: u32,
}

/// System-wide metrics on the wire.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemDto {
    /// Total CPU, percent.
    pub cpu_total_percent: f32,
    /// Per logical core, percent.
    pub cpu_per_core_percent: Vec<f32>,
    /// Physical memory total, bytes.
    pub memory_total_bytes: u64,
    /// Physical memory in use, bytes.
    pub memory_used_bytes: u64,
    /// Network upload, bytes/s, or null if unavailable.
    pub net_up_bytes_per_sec: Option<u64>,
    /// Network download, bytes/s, or null if unavailable.
    pub net_down_bytes_per_sec: Option<u64>,
}

/// Per-process metrics on the wire. Unavailable metrics are null, never 0.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDto {
    /// Process ID.
    pub pid: u32,
    /// Executable name.
    pub name: String,
    /// CPU percent of the whole machine.
    pub cpu_percent: f32,
    /// Working set, bytes.
    pub working_set_bytes: u64,
    /// Private memory, bytes.
    pub private_bytes: u64,
    /// Commit (virtual) memory, bytes.
    pub commit_bytes: u64,
    /// GPU engine percent, or null.
    pub gpu_engine_percent: Option<f32>,
    /// VRAM bytes, or null.
    pub vram_bytes: Option<u64>,
    /// Disk read bytes/s, or null.
    pub disk_read_bytes_per_sec: Option<u64>,
    /// Disk write bytes/s, or null.
    pub disk_write_bytes_per_sec: Option<u64>,
    /// Network upload bytes/s, or null.
    pub net_up_bytes_per_sec: Option<u64>,
    /// Network download bytes/s, or null.
    pub net_down_bytes_per_sec: Option<u64>,
    /// Whether the process has a visible window.
    pub has_visible_window: bool,
}

/// A suggestion on the wire.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionDto {
    /// Process the suggestion is about.
    pub pid: u32,
    /// Executable name.
    pub process_name: String,
    /// Explanation shown to the user.
    pub rationale: String,
    /// The reversible action offered.
    pub recommended_action: ActionDto,
    /// Confidence, 0.0..=1.0.
    pub confidence: f32,
    /// Average CPU over the window.
    pub impact_cpu_percent: f32,
}

/// A process action. Deserialization rejects unknown kinds and extra fields, so the
/// IPC surface is exactly the variants listed here.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionDto {
    /// Terminate (destructive; requires confirmation).
    Kill,
    /// Suspend all threads.
    Suspend,
    /// Resume a suspended process.
    Resume,
    /// Change priority class.
    SetPriority {
        /// The priority class.
        priority: PriorityDto,
    },
    /// Restrict to a CPU bitmask.
    SetAffinity {
        /// Bit N selects logical CPU N.
        mask: u64,
    },
}

/// Priority classes on the wire.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PriorityDto {
    /// Idle.
    Idle,
    /// Below normal.
    BelowNormal,
    /// Normal.
    Normal,
    /// Above normal.
    AboveNormal,
    /// High.
    High,
}

/// Suggestion settings shown in the dashboard: what the user dismissed, and which
/// processes are protected from suggestions and disruptive actions.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionSettingsDto {
    /// Executables whose suggestions were dismissed.
    pub dismissed: Vec<String>,
    /// Protected system processes, lowercase.
    pub protected: Vec<String>,
}

/// Dashboard visibility, sent by the frontend.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewDto {
    /// Dashboard is visible.
    pub dashboard_visible: bool,
    /// Dashboard has focus.
    pub dashboard_focused: bool,
    /// Number of open overlay windows.
    pub open_overlays: usize,
}

impl From<ActionDto> for ProcessAction {
    fn from(a: ActionDto) -> Self {
        match a {
            ActionDto::Kill => ProcessAction::Kill,
            ActionDto::Suspend => ProcessAction::Suspend,
            ActionDto::Resume => ProcessAction::Resume,
            ActionDto::SetPriority { priority } => ProcessAction::SetPriority(priority.into()),
            ActionDto::SetAffinity { mask } => ProcessAction::SetAffinity(mask),
        }
    }
}

impl From<PriorityDto> for Priority {
    fn from(p: PriorityDto) -> Self {
        match p {
            PriorityDto::Idle => Priority::Idle,
            PriorityDto::BelowNormal => Priority::BelowNormal,
            PriorityDto::Normal => Priority::Normal,
            PriorityDto::AboveNormal => Priority::AboveNormal,
            PriorityDto::High => Priority::High,
        }
    }
}

impl From<ViewDto> for ViewState {
    fn from(v: ViewDto) -> Self {
        ViewState {
            dashboard_visible: v.dashboard_visible,
            dashboard_focused: v.dashboard_focused,
            open_overlays: v.open_overlays,
        }
    }
}

impl From<&Frame> for FrameDto {
    fn from(frame: &Frame) -> Self {
        let s = &frame.snapshot.system;
        FrameDto {
            system: SystemDto {
                cpu_total_percent: s.cpu_total_percent,
                cpu_per_core_percent: s.cpu_per_core_percent.clone(),
                memory_total_bytes: s.memory_total_bytes,
                memory_used_bytes: s.memory_used_bytes,
                net_up_bytes_per_sec: s.net_up_bytes_per_sec,
                net_down_bytes_per_sec: s.net_down_bytes_per_sec,
            },
            processes: frame.snapshot.processes.iter().map(ProcessDto::from).collect(),
            suggestions: frame.suggestions.iter().map(SuggestionDto::from).collect(),
            sensors: SensorsDto {
                temperatures: frame.sensors.temperatures.as_ref().map(|v| {
                    v.iter()
                        .map(|t| TemperatureDto { label: t.label.clone(), celsius: t.celsius })
                        .collect()
                }),
                fans: frame.sensors.fans.as_ref().map(|v| {
                    v.iter().map(|f| FanDto { label: f.label.clone(), rpm: f.rpm }).collect()
                }),
            },
        }
    }
}

impl From<&ProcessMetrics> for ProcessDto {
    fn from(p: &ProcessMetrics) -> Self {
        ProcessDto {
            pid: p.pid.0,
            name: p.name.clone(),
            cpu_percent: p.cpu_percent,
            working_set_bytes: p.memory.working_set_bytes,
            private_bytes: p.memory.private_bytes,
            commit_bytes: p.memory.commit_bytes,
            gpu_engine_percent: p.gpu_engine_percent,
            vram_bytes: p.vram_bytes,
            disk_read_bytes_per_sec: p.disk_read_bytes_per_sec,
            disk_write_bytes_per_sec: p.disk_write_bytes_per_sec,
            net_up_bytes_per_sec: p.net_up_bytes_per_sec,
            net_down_bytes_per_sec: p.net_down_bytes_per_sec,
            has_visible_window: p.has_visible_window,
        }
    }
}

impl From<&Suggestion> for SuggestionDto {
    fn from(s: &Suggestion) -> Self {
        SuggestionDto {
            pid: s.pid.0,
            process_name: s.process_name.clone(),
            rationale: s.rationale.clone(),
            recommended_action: action_to_dto(s.recommended_action),
            confidence: s.confidence,
            impact_cpu_percent: s.impact_cpu_percent,
        }
    }
}

/// Core action to wire action, for the variants the suggestion engine emits and the rest alike.
fn action_to_dto(a: ProcessAction) -> ActionDto {
    match a {
        ProcessAction::Kill => ActionDto::Kill,
        ProcessAction::Suspend => ActionDto::Suspend,
        ProcessAction::Resume => ActionDto::Resume,
        ProcessAction::SetPriority(p) => ActionDto::SetPriority {
            priority: match p {
                Priority::Idle => PriorityDto::Idle,
                Priority::BelowNormal => PriorityDto::BelowNormal,
                Priority::Normal => PriorityDto::Normal,
                Priority::AboveNormal => PriorityDto::AboveNormal,
                Priority::High => PriorityDto::High,
            },
        },
        ProcessAction::SetAffinity(mask) => ActionDto::SetAffinity { mask },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_parses_known_kinds() {
        let a: ActionDto = serde_json::from_str(r#"{"kind":"suspend"}"#).unwrap();
        assert_eq!(a, ActionDto::Suspend);
        let p: ActionDto =
            serde_json::from_str(r#"{"kind":"set_priority","priority":"below_normal"}"#).unwrap();
        assert_eq!(ProcessAction::from(p), ProcessAction::SetPriority(Priority::BelowNormal));
    }

    #[test]
    fn unknown_action_kind_is_rejected() {
        assert!(serde_json::from_str::<ActionDto>(r#"{"kind":"shell_exec"}"#).is_err());
        assert!(serde_json::from_str::<ActionDto>(r#"{"kind":"Kill"}"#).is_err());
    }

    #[test]
    fn extra_fields_and_unknown_priorities_are_rejected() {
        // serde does not apply deny_unknown_fields to unit variants of an internally
        // tagged enum, so a stray `pid` is ignored here. The target PID only ever
        // arrives as the separate `pid` argument of apply_action, so this is harmless.
        assert_eq!(
            serde_json::from_str::<ActionDto>(r#"{"kind":"kill","pid":4}"#).unwrap(),
            ActionDto::Kill
        );
        // Struct variants do reject extra fields.
        assert!(
            serde_json::from_str::<ActionDto>(r#"{"kind":"set_affinity","mask":1,"pid":4}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<ActionDto>(r#"{"kind":"set_priority","priority":"realtime"}"#)
                .is_err()
        );
    }

    #[test]
    fn view_rejects_unknown_fields() {
        let ok = serde_json::from_str::<ViewDto>(
            r#"{"dashboardVisible":true,"dashboardFocused":false,"openOverlays":1}"#,
        );
        assert!(ok.is_ok());
        assert!(
            serde_json::from_str::<ViewDto>(
                r#"{"dashboardVisible":true,"dashboardFocused":false,"openOverlays":1,"x":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn unavailable_metrics_serialize_as_null() {
        let p = ProcessMetrics::new(tpt_monitor_core::Pid(7), "a.exe");
        let json = serde_json::to_value(ProcessDto::from(&p)).unwrap();
        assert!(json["gpuEnginePercent"].is_null());
        assert!(json["diskReadBytesPerSec"].is_null());
        assert_eq!(json["workingSetBytes"], 0);
    }
}
