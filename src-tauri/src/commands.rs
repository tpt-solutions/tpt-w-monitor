// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Tauri command handlers. Each one locks the shared state, delegates to
//! [`crate::state::MonitorState`] (which enforces the rules), and maps errors to strings.

use std::sync::MutexGuard;

use tauri::State;
use tpt_monitor_core::Pid;

use crate::dto::{ActionDto, FrameDto, ViewDto};
use crate::state::MonitorState;
use crate::{AppState, Backend};

fn lock(state: &AppState) -> MutexGuard<'_, MonitorState<Backend>> {
    // A poisoned lock only means a previous holder panicked; the state is still usable.
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The most recent frame, or null before the first sample.
#[tauri::command]
pub fn get_frame(state: State<'_, AppState>) -> Option<FrameDto> {
    lock(&state).latest().map(FrameDto::from)
}

/// Applies a process action to a PID from the latest snapshot.
#[tauri::command]
pub fn apply_action(
    pid: u32,
    action: ActionDto,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    lock(&state).apply_action(Pid(pid), action.into(), confirmed).map_err(|e| e.to_string())
}

/// Tells the sampler what the user can see.
#[tauri::command]
pub fn set_view_state(view: ViewDto, state: State<'_, AppState>) {
    lock(&state).set_view_state(view.into());
}

/// Stops suggestions for an executable ("don't suggest again").
#[tauri::command]
pub fn dismiss_suggestion(process_name: String, state: State<'_, AppState>) {
    lock(&state).dismiss_suggestion(&process_name);
}

/// Re-enables suggestions for an executable.
#[tauri::command]
pub fn restore_suggestion(process_name: String, state: State<'_, AppState>) {
    lock(&state).restore_suggestion(&process_name);
}

fn layouts_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    app.path().app_data_dir().map(|dir| dir.join("layouts")).map_err(|e| e.to_string())
}

/// Saves a dashboard layout (JSON) under `name` in the app data folder.
#[tauri::command]
pub fn save_layout(app: tauri::AppHandle, name: String, content: String) -> Result<(), String> {
    crate::layouts::save(&layouts_dir(&app)?, &name, &content)
}

/// Loads a saved layout, or null if none has that name.
#[tauri::command]
pub fn load_layout(app: tauri::AppHandle, name: String) -> Result<Option<String>, String> {
    crate::layouts::load(&layouts_dir(&app)?, &name)
}

/// Names of all saved layouts.
#[tauri::command]
pub fn list_layouts(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    crate::layouts::list(&layouts_dir(&app)?)
}
