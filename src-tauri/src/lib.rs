// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! tpt-monitor Tauri shell: runs the sampling loop on a background thread, pushes
//! frames to the window as `metrics` events, serves IPC commands, and manages
//! desktop overlay windows and the system tray.

pub mod commands;
pub mod dto;
pub mod layouts;
pub mod overlays;
pub mod protected;
pub mod state;
pub mod tray;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tpt_monitor_core::CriticalList;

use crate::dto::FrameDto;
use crate::overlays::{DEFAULT_SIZE, OverlayConfig, OverlayStore};
use crate::state::MonitorState;

#[cfg(windows)]
/// The OS backend used by the app.
pub type Backend = tpt_monitor_platform_windows::WindowsMonitor;
#[cfg(target_os = "linux")]
/// The OS backend used by the app.
pub type Backend = tpt_monitor_platform_linux::LinuxMonitor;
#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("tpt-monitor supports Windows and Linux only (macOS is backlog)");

/// The OS sensor provider used by the app.
#[cfg(windows)]
fn make_sensors() -> Box<dyn tpt_monitor_core::SensorProvider + Send> {
    Box::new(tpt_monitor_platform_windows::WindowsSensors)
}

/// The OS sensor provider used by the app.
#[cfg(target_os = "linux")]
fn make_sensors() -> Box<dyn tpt_monitor_core::SensorProvider + Send> {
    Box::new(tpt_monitor_platform_linux::LinuxSensors::new())
}

/// Shared backend state, managed by Tauri.
pub type AppState = Mutex<MonitorState<Backend>>;

/// Open overlays and their saved positions, managed by Tauri.
pub type OverlayState = Mutex<OverlayStore>;

/// Frames are never emitted faster than this, however fast the sampler runs.
pub const MIN_EMIT_INTERVAL: Duration = Duration::from_millis(250);

/// Event sent by an overlay when the user docks it back into the dashboard.
pub const EVENT_OVERLAY_DOCK: &str = "overlay-dock";

fn critical_list() -> CriticalList {
    #[cfg(windows)]
    {
        CriticalList::windows_default()
    }
    #[cfg(target_os = "linux")]
    {
        CriticalList::linux_default()
    }
}

/// Window label for an overlay. Ids are validated before they reach this point.
pub fn overlay_label(id: &str) -> String {
    format!("overlay-{id}")
}

/// Creates (or re-shows) the overlay window for `config`.
///
/// Overlays are borderless, always on top, hidden from the taskbar and transparent,
/// so the widget floats over the desktop.
pub fn spawn_overlay_window(
    app: &tauri::AppHandle,
    config: &OverlayConfig,
) -> tauri::Result<WebviewWindow> {
    let label = overlay_label(&config.id);
    if let Some(existing) = app.get_webview_window(&label) {
        existing.show()?;
        return Ok(existing);
    }
    let url = format!("index.html?overlay={}&id={}", config.kind, config.id);
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(format!("tpt-monitor {}", config.kind))
        .inner_size(f64::from(config.width), f64::from(config.height))
        .position(f64::from(config.x), f64::from(config.y))
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .shadow(false)
        .build()?;
    if config.click_through {
        window.set_ignore_cursor_events(true)?;
    }
    Ok(window)
}

/// Turns click-through on or off for every open overlay. If any overlay is currently
/// interactive, all of them become click-through; otherwise all become interactive.
pub fn toggle_all_click_through(app: &tauri::AppHandle) {
    let Some(overlays) = app.try_state::<OverlayState>() else {
        return;
    };
    let mut store = overlays.lock().unwrap_or_else(|p| p.into_inner());
    let enable = store.list().iter().any(|c| !c.click_through);
    let ids: Vec<String> = store.list().into_iter().map(|c| c.id).collect();
    for id in ids {
        if let Some(window) = app.get_webview_window(&overlay_label(&id)) {
            let _ = window.set_ignore_cursor_events(enable);
        }
        if let Some(config) = store.get_mut(&id) {
            config.click_through = enable;
        }
    }
    let _ = store.save();
}

/// Default placement for a newly opened overlay: a cascade so new ones do not stack.
pub fn default_overlay_placement(existing: usize) -> (i32, i32, u32, u32) {
    let step = 32 * existing as i32;
    (120 + step, 120 + step, DEFAULT_SIZE.0, DEFAULT_SIZE.1)
}

/// Starts the app: sampling thread, IPC handlers, tray, overlays, and the main window.
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(MonitorState::new(Backend::new(), critical_list(), make_sensors())))
        .invoke_handler(tauri::generate_handler![
            commands::get_frame,
            commands::apply_action,
            commands::set_view_state,
            commands::dismiss_suggestion,
            commands::restore_suggestion,
            commands::get_suggestion_settings,
            commands::get_capabilities,
            commands::add_protected,
            commands::remove_protected,
            commands::save_layout,
            commands::load_layout,
            commands::list_layouts,
            commands::open_overlay,
            commands::close_overlay,
            commands::update_overlay,
            commands::set_click_through,
            commands::list_overlays,
        ])
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            // User additions to the protected list apply before anything is sampled.
            {
                let state = app.state::<AppState>();
                let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
                for name in protected::load(&dir.join("protected.json")) {
                    guard.protect_process(&name);
                }
            }
            let store = OverlayStore::load(dir.join("overlays.json"));
            let restore = store.list();
            app.manage(Mutex::new(store));

            let handles = tray::install(app.handle())?;
            app.manage(handles);

            for config in &restore {
                if let Err(err) = spawn_overlay_window(app.handle(), config) {
                    eprintln!("could not restore overlay {}: {err}", config.id);
                }
            }

            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("sampler".into())
                .spawn(move || sampling_loop(handle))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tpt-monitor");
}

fn sampling_loop(handle: tauri::AppHandle) {
    loop {
        let state = handle.state::<AppState>();
        let (frame, cpu, memory, top, wait) = {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            match guard.tick() {
                Ok(frame) => {
                    let frame = FrameDto::from(frame);
                    let series =
                        (guard.cpu_history(), guard.memory_history(), guard.top_processes(3));
                    (Some(frame), series.0, series.1, series.2, guard.next_interval())
                }
                Err(err) => {
                    eprintln!("sampling failed: {err}");
                    (None, Vec::new(), Vec::new(), Vec::new(), guard.next_interval())
                }
            }
        };
        if let Some(frame) = frame {
            let _ = handle.emit("metrics", frame);
        }
        if let Some(handles) = handle.try_state::<tray::TrayHandles>() {
            let _ = tray::update(&handles, &cpu, &memory, &top);
        }
        std::thread::sleep(wait.max(MIN_EMIT_INTERVAL));
    }
}

/// Platform name reported to the frontend.
#[cfg(windows)]
pub const PLATFORM: &str = "windows";
/// Platform name reported to the frontend.
#[cfg(target_os = "linux")]
pub const PLATFORM: &str = "linux";

/// Whether this process is elevated on the current platform.
pub fn platform_elevated() -> bool {
    #[cfg(windows)]
    {
        tpt_monitor_platform_windows::is_elevated()
    }
    #[cfg(target_os = "linux")]
    {
        tpt_monitor_platform_linux::is_elevated()
    }
}
