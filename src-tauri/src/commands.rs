// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Tauri command handlers. Each one locks the shared state, delegates to
//! [`crate::state::MonitorState`] (which enforces the rules), and maps errors to strings.

use std::sync::MutexGuard;

use tauri::{Manager, State};
use tpt_monitor_core::Pid;

use crate::dto::{ActionDto, FrameDto, SuggestionSettingsDto, ViewDto};
use crate::overlays::{
    OverlayConfig, OverlayStore, clamp_opacity, clamp_size, validate_id, validate_kind,
};
use crate::state::MonitorState;
use crate::{
    AppState, Backend, OverlayState, default_overlay_placement, overlay_label, spawn_overlay_window,
};

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

/// What suggestions have been dismissed, and which processes are protected.
#[tauri::command]
pub fn get_suggestion_settings(state: State<'_, AppState>) -> SuggestionSettingsDto {
    let guard = lock(&state);
    SuggestionSettingsDto {
        dismissed: guard.dismissed_suggestions(),
        protected: guard.protected_processes(),
    }
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

fn lock_overlays(state: &OverlayState) -> std::sync::MutexGuard<'_, OverlayStore> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Pops a widget out into its own always-on-top window. `id` is the widget id from
/// the dashboard, so the same widget can be docked back to where it came from.
#[tauri::command]
pub fn open_overlay(
    app: tauri::AppHandle,
    kind: String,
    id: String,
    overlays: State<'_, OverlayState>,
) -> Result<OverlayConfig, String> {
    validate_kind(&kind)?;
    validate_id(&id)?;
    let mut store = lock_overlays(&overlays);
    let (x, y, width, height) = default_overlay_placement(store.list().len());
    let config = OverlayConfig {
        id: id.clone(),
        kind,
        x,
        y,
        width,
        height,
        opacity: 1.0,
        click_through: false,
    };
    store.insert(config.clone())?;
    if let Err(err) = spawn_overlay_window(&app, &config) {
        store.remove(&id);
        return Err(err.to_string());
    }
    store.save()?;
    Ok(config)
}

/// Closes an overlay window and forgets it.
#[tauri::command]
pub fn close_overlay(
    app: tauri::AppHandle,
    id: String,
    overlays: State<'_, OverlayState>,
) -> Result<(), String> {
    validate_id(&id)?;
    let mut store = lock_overlays(&overlays);
    store.remove(&id);
    if let Some(window) = app.get_webview_window(&overlay_label(&id)) {
        window.close().map_err(|e| e.to_string())?;
    }
    store.save()
}

/// Saves an overlay's position, size and opacity. Values are clamped, so a bad request
/// cannot produce an invisible or absurdly large window.
#[tauri::command]
pub fn update_overlay(
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    opacity: f32,
    overlays: State<'_, OverlayState>,
) -> Result<(), String> {
    validate_id(&id)?;
    let mut store = lock_overlays(&overlays);
    let config = store.get_mut(&id).ok_or_else(|| format!("overlay {id} is not open"))?;
    let (width, height) = clamp_size(width, height);
    config.x = x;
    config.y = y;
    config.width = width;
    config.height = height;
    config.opacity = clamp_opacity(opacity);
    store.save()
}

/// Makes one overlay pass mouse input through to the desktop, or takes it back.
#[tauri::command]
pub fn set_click_through(
    app: tauri::AppHandle,
    id: String,
    enabled: bool,
    overlays: State<'_, OverlayState>,
) -> Result<(), String> {
    validate_id(&id)?;
    let mut store = lock_overlays(&overlays);
    let config = store.get_mut(&id).ok_or_else(|| format!("overlay {id} is not open"))?;
    let window = app
        .get_webview_window(&overlay_label(&id))
        .ok_or_else(|| format!("overlay {id} has no window"))?;
    window.set_ignore_cursor_events(enabled).map_err(|e| e.to_string())?;
    config.click_through = enabled;
    store.save()
}

/// All open overlays, in id order.
#[tauri::command]
pub fn list_overlays(overlays: State<'_, OverlayState>) -> Vec<OverlayConfig> {
    lock_overlays(&overlays).list()
}
