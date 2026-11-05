// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Error type shared by backends, the action gate and the sampler.

use std::fmt;

use crate::action::ProcessAction;
use crate::model::Pid;

/// Errors returned by monitor backends and the action layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorError {
    /// The backend or feature is not available on this platform.
    Unsupported(&'static str),
    /// No process with this PID exists (it may have exited).
    ProcessNotFound(Pid),
    /// The OS refused the operation; usually means the app is not elevated.
    PermissionDenied(Pid),
    /// A destructive action was requested without explicit confirmation.
    ConfirmationRequired(ProcessAction),
    /// The action's arguments are invalid, e.g. an empty affinity mask.
    InvalidAction(&'static str),
    /// Any other OS-level failure, with the OS message.
    Os(String),
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "not supported on this platform: {what}"),
            Self::ProcessNotFound(pid) => write!(f, "process {pid} not found"),
            Self::PermissionDenied(pid) => {
                write!(f, "permission denied for process {pid} (try running elevated)")
            }
            Self::ConfirmationRequired(action) => {
                write!(f, "action {action:?} is destructive and requires confirmation")
            }
            Self::InvalidAction(reason) => write!(f, "invalid action: {reason}"),
            Self::Os(msg) => write!(f, "OS error: {msg}"),
        }
    }
}

impl std::error::Error for MonitorError {}

/// Convenience alias used across the core and backends.
pub type Result<T> = std::result::Result<T, MonitorError>;
