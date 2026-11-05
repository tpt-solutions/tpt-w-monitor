// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Process actions and the confirmation gate.
//!
//! Every action goes through [`execute_action`], so the "destructive actions need
//! confirmation" rule is enforced in the core, not just in the UI.

use crate::error::{MonitorError, Result};
use crate::model::Pid;
use crate::traits::SystemMonitor;

/// Scheduling priority classes. Real-time is deliberately not offered: it can
/// starve the whole system and the monitor should never be the cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Lowest priority class.
    Idle,
    /// Below normal.
    BelowNormal,
    /// Default class.
    Normal,
    /// Above normal.
    AboveNormal,
    /// High priority class.
    High,
}

/// An action the user (or, later, a suggestion) can apply to a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessAction {
    /// Terminate the process. Destructive: unsaved state is lost.
    Kill,
    /// Pause all threads (`NtSuspendProcess` / `SIGSTOP`). Reversible.
    Suspend,
    /// Resume a suspended process (`NtResumeProcess` / `SIGCONT`).
    Resume,
    /// Change scheduling priority.
    SetPriority(Priority),
    /// Restrict execution to the CPUs set in this bitmask (bit N = logical CPU N).
    SetAffinity(u64),
}

impl ProcessAction {
    /// Whether this action can lose user data and therefore needs explicit
    /// confirmation before it runs.
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, Self::Kill)
    }

    /// Checks the arguments without touching the OS.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::SetAffinity(0) => {
                Err(MonitorError::InvalidAction("affinity mask must select at least one CPU"))
            }
            _ => Ok(()),
        }
    }
}

/// Validates and runs `action` against `pid`.
///
/// `confirmed` must be `true` for destructive actions; otherwise the backend is
/// never called and [`MonitorError::ConfirmationRequired`] is returned.
pub fn execute_action<M: SystemMonitor + ?Sized>(
    monitor: &mut M,
    pid: Pid,
    action: ProcessAction,
    confirmed: bool,
) -> Result<()> {
    action.validate()?;
    if action.requires_confirmation() && !confirmed {
        return Err(MonitorError::ConfirmationRequired(action));
    }
    monitor.apply_action(pid, action)
}
