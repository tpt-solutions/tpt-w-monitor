// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! System tray: a live icon with CPU and memory bar graphs, the top processes as menu
//! lines, and quick actions (open dashboard, click-through, quit).

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Result as TauriResult};

/// Icon edge length in pixels. Windows scales tray icons to 16-32 px; 32 keeps the graphs legible.
pub const ICON_SIZE: u32 = 32;

/// Menu item ids. Kept together so the event handler and the builder cannot drift apart.
pub const MENU_OPEN: &str = "tray-open";
/// See [`MENU_OPEN`].
pub const MENU_CLICK_THROUGH: &str = "tray-click-through";
/// See [`MENU_OPEN`].
pub const MENU_QUIT: &str = "tray-quit";
const MENU_TOP_PREFIX: &str = "tray-top-";

/// Handles kept so the tray can be refreshed each sample.
pub struct TrayHandles {
    icon: TrayIcon,
    top: Vec<MenuItem<tauri::Wry>>,
}

const BAR: [u8; 4] = [0x6f, 0xb6, 0xff, 0xff];
const TRACK: [u8; 4] = [0x2c, 0x38, 0x45, 0xff];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// Draws two bar graphs side by side on a transparent square: CPU on the left half and
/// memory on the right. Each half shows its most recent samples, newest on the right
/// edge of that half. Unfilled space shows a faint track once a sample exists.
/// The output is RGBA, `size * size * 4` bytes.
pub fn render_dual(cpu: &[f32], memory: &[f32], size: u32) -> Vec<u8> {
    let size = size.max(2) as usize;
    let mut px = vec![0u8; size * size * 4];
    let left_width = size / 2;
    draw_series(&mut px, size, 0, left_width, cpu);
    draw_series(&mut px, size, left_width, size - left_width, memory);
    px
}

/// Draws `values` into the column range `[x0, x0 + width)`.
fn draw_series(px: &mut [u8], size: usize, x0: usize, width: usize, values: &[f32]) {
    let start = values.len().saturating_sub(width);
    let recent = &values[start..];
    let offset = width - recent.len();

    for column in 0..width {
        let value = if column < offset {
            None
        } else {
            recent.get(column - offset).map(|v| v.clamp(0.0, 100.0))
        };
        let filled = value.map(|v| ((v / 100.0) * size as f32).round() as usize).unwrap_or(0);
        let x = x0 + column;
        for y in 0..size {
            let from_bottom = size - 1 - y;
            let colour = if from_bottom < filled {
                BAR
            } else if value.is_some() {
                TRACK
            } else {
                CLEAR
            };
            let i = (y * size + x) * 4;
            px[i..i + 4].copy_from_slice(&colour);
        }
    }
}

/// Builds the tray icon and menu. Call once during setup.
pub fn install(app: &AppHandle) -> TauriResult<TrayHandles> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Open dashboard", true, None::<&str>)?;
    let mut top = Vec::new();
    for i in 0..3 {
        let item = MenuItem::with_id(
            app,
            format!("{MENU_TOP_PREFIX}{i}"),
            "Top process: —",
            false,
            None::<&str>,
        )?;
        top.push(item);
    }
    let click = MenuItem::with_id(
        app,
        MENU_CLICK_THROUGH,
        "Toggle click-through for overlays",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit tpt-monitor", true, None::<&str>)?;

    let separator = PredefinedMenuItem::separator(app)?;
    let mut items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = vec![&open, &separator];
    for item in &top {
        items.push(item);
    }
    items.push(&separator);
    items.push(&click);
    items.push(&quit);
    let menu = Menu::with_items(app, &items)?;

    let icon = TrayIconBuilder::with_id("main-tray")
        .icon(Image::new_owned(render_dual(&[], &[], ICON_SIZE), ICON_SIZE, ICON_SIZE))
        .tooltip("tpt-monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => show_main_window(app),
            MENU_CLICK_THROUGH => crate::toggle_all_click_through(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(TrayHandles { icon, top })
}

/// Brings the dashboard to the front, creating focus if the window was hidden.
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Refreshes the icon graphs, the tooltip, and the top-process lines.
/// `memory_history` is percent in use, like `cpu_history`.
pub fn update(
    handles: &TrayHandles,
    cpu_history: &[f32],
    memory_history: &[f32],
    top: &[(String, f32)],
) -> TauriResult<()> {
    let rgba = render_dual(cpu_history, memory_history, ICON_SIZE);
    handles.icon.set_icon(Some(Image::new_owned(rgba, ICON_SIZE, ICON_SIZE)))?;
    let cpu = cpu_history.last().copied().unwrap_or(0.0);
    let memory = memory_history.last().copied().unwrap_or(0.0);
    handles.icon.set_tooltip(Some(format!("tpt-monitor · CPU {cpu:.0}% · Memory {memory:.0}%")))?;
    for (i, item) in handles.top.iter().enumerate() {
        let text = match top.get(i) {
            Some((name, cpu)) => format!("{name} · {cpu:.0}%"),
            None => "Top process: —".to_string(),
        };
        item.set_text(text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(px: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn output_has_the_expected_size() {
        assert_eq!(render_dual(&[10.0, 20.0], &[], 32).len(), 32 * 32 * 4);
        assert_eq!(render_dual(&[], &[], 0).len(), 2 * 2 * 4);
    }

    #[test]
    fn no_history_is_fully_transparent() {
        let px = render_dual(&[], &[], 16);
        assert!(px.chunks(4).all(|p| p == CLEAR));
    }

    #[test]
    fn cpu_fills_the_left_half_and_memory_the_right() {
        let size = 8;
        let px = render_dual(&[100.0; 4], &[0.0; 4], size);
        // Left half (columns 0..4): full CPU load fills every row of the latest column.
        for y in 0..size {
            assert_eq!(pixel(&px, size, 3, y), BAR, "cpu row {y}");
        }
        // Right half (columns 4..8): memory at zero is a track, not a bar.
        assert_eq!(pixel(&px, size, 7, size - 1), TRACK);
        assert_eq!(pixel(&px, size, 7, 0), TRACK);
    }

    #[test]
    fn half_memory_load_fills_the_bottom_half() {
        let size = 8;
        let px = render_dual(&[], &[50.0; 8], size);
        assert_eq!(pixel(&px, size, 6, size - 1), BAR);
        assert_eq!(pixel(&px, size, 6, 0), TRACK);
    }

    #[test]
    fn short_history_is_right_aligned_within_its_half() {
        let size = 8;
        let px = render_dual(&[100.0], &[], size);
        // One CPU sample: only the newest column of the left half has data.
        assert_eq!(pixel(&px, size, 3, size - 1), BAR);
        assert_eq!(pixel(&px, size, 0, size - 1), CLEAR);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let size = 4;
        let high = render_dual(&[500.0], &[], size);
        let low = render_dual(&[-40.0], &[], size);
        assert_eq!(pixel(&high, size, 1, 0), BAR);
        assert_eq!(pixel(&low, size, 1, 3), TRACK);
    }
}
