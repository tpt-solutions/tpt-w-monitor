// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! tpt-monitor Tauri shell: runs the sampling loop on a background thread, pushes
//! frames to the window as `metrics` events, and serves IPC commands.

pub mod commands;
pub mod dto;
pub mod layouts;
pub mod state;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{Emitter, Manager};
use tpt_monitor_core::CriticalList;

use crate::dto::FrameDto;
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

/// Frames are never emitted faster than this, however fast the sampler runs.
pub const MIN_EMIT_INTERVAL: Duration = Duration::from_millis(250);

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

/// Starts the app: sampling thread, IPC handlers, and the main window.
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(MonitorState::new(Backend::new(), critical_list(), make_sensors())))
        .invoke_handler(tauri::generate_handler![
            commands::get_frame,
            commands::apply_action,
            commands::set_view_state,
            commands::dismiss_suggestion,
            commands::restore_suggestion,
            commands::save_layout,
            commands::load_layout,
            commands::list_layouts,
        ])
        .setup(|app| {
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
        let (frame, wait) = {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            let frame = match guard.tick() {
                Ok(frame) => Some(FrameDto::from(frame)),
                Err(err) => {
                    eprintln!("sampling failed: {err}");
                    None
                }
            };
            (frame, guard.next_interval())
        };
        if let Some(frame) = frame {
            let _ = handle.emit("metrics", frame);
        }
        std::thread::sleep(wait.max(MIN_EMIT_INTERVAL));
    }
}
