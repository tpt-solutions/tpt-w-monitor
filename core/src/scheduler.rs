// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Adaptive sampling interval: sample fast when the user is watching, slow down
//! when nothing visible needs fresh data.

use std::time::Duration;

/// What the user can currently see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewState {
    /// The dashboard window exists and is visible on screen.
    pub dashboard_visible: bool,
    /// The dashboard window has keyboard focus.
    pub dashboard_focused: bool,
    /// Number of desktop overlay windows currently open.
    pub open_overlays: usize,
}

/// Chooses the sampling interval from the base interval and the view state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplingScheduler {
    base: Duration,
    max: Duration,
    view: ViewState,
}

impl SamplingScheduler {
    /// Creates a scheduler. If `max` is shorter than `base`, `max` is raised to `base`.
    pub fn new(base: Duration, max: Duration) -> Self {
        Self { base, max: max.max(base), view: ViewState::default() }
    }

    /// Updates what the user can see; takes effect on the next [`Self::current_interval`].
    pub fn set_view_state(&mut self, view: ViewState) {
        self.view = view;
    }

    /// Current view state.
    pub fn view_state(&self) -> ViewState {
        self.view
    }

    /// The interval to wait before the next sample.
    ///
    /// - focused dashboard: base interval
    /// - overlays open (dashboard unfocused or hidden): 2x
    /// - visible but unfocused dashboard, no overlays: 4x
    /// - hidden or minimized, no overlays: 8x
    ///
    /// Capped at the configured maximum.
    pub fn current_interval(&self) -> Duration {
        let factor = if self.view.dashboard_visible && self.view.dashboard_focused {
            1
        } else if self.view.open_overlays > 0 {
            2
        } else if self.view.dashboard_visible {
            4
        } else {
            8
        };
        self.base.saturating_mul(factor).min(self.max)
    }
}
