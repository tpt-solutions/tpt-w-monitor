// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! System tray: a live mini CPU graph as the icon, the top processes as menu
//! lines, and quick actions (open dashboard, click-through, quit).

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Result as TauriResult};

/// Icon edge length in pixels. Windows scales tray icons to 16-32 px; 32 keeps the graph legible.
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

/// Draws the last `size` CPU samples as a bar chart on a transparent square.
///
/// Each bar is one pixel column wide and reaches the height of its sample. The
/// remaining space shows a faint track so an idle machine still looks like a graph.
/// The output is RGBA, `size * size * 4` bytes.
pub fn render_graph(history: &[f32], size: u32) -> Vec<u8> {
    let size = size.max(1) as usize;
    let mut px = vec![0u8; size * size * 4];
    let columns = size;
    // Most recent sample goes on the right edge.
    let start = history.len().saturating_sub(columns);
    let recent = &history[start..];
    let offset = columns - recent.len();

    for x in 0..columns {
        let value =
            if x < offset { None } else { recent.get(x - offset).map(|v| v.clamp(0.0, 100.0)) };
        let filled = value.map(|v| ((v / 100.0) * size as f32).round() as usize).unwrap_or(0);
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
    px
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
        .icon(Image::new_owned(render_graph(&[], ICON_SIZE), ICON_SIZE, ICON_SIZE))
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

/// Refreshes the icon graph and the top-process lines.
pub fn update(
    handles: &TrayHandles,
    cpu_history: &[f32],
    top: &[(String, f32)],
) -> TauriResult<()> {
    let image = Image::new_owned(render_graph(cpu_history, ICON_SIZE), ICON_SIZE, ICON_SIZE);
    handles.icon.set_icon(Some(image))?;
    let current = cpu_history.last().copied().unwrap_or(0.0);
    handles.icon.set_tooltip(Some(format!("tpt-monitor · CPU {current:.0}%")))?;
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
        assert_eq!(render_graph(&[10.0, 20.0], 32).len(), 32 * 32 * 4);
        assert_eq!(render_graph(&[], 0).len(), 4);
    }

    #[test]
    fn empty_history_is_fully_transparent() {
        let px = render_graph(&[], 16);
        assert!(px.chunks(4).all(|p| p == CLEAR));
    }

    #[test]
    fn full_load_fills_the_latest_column_to_the_top() {
        let size = 8;
        let px = render_graph(&[100.0], size);
        // Latest sample is the rightmost column; full load fills every row.
        for y in 0..size {
            assert_eq!(pixel(&px, size, size - 1, y), BAR, "row {y}");
        }
        // Columns with no data stay transparent.
        assert_eq!(pixel(&px, size, 0, size - 1), CLEAR);
    }

    #[test]
    fn half_load_fills_the_bottom_half_and_shows_track_above() {
        let size = 8;
        let px = render_graph(&[50.0; 8], size);
        assert_eq!(pixel(&px, size, 3, size - 1), BAR);
        assert_eq!(pixel(&px, size, 3, 0), TRACK);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let size = 4;
        let high = render_graph(&[500.0], size);
        let low = render_graph(&[-40.0], size);
        assert_eq!(pixel(&high, size, 3, 0), BAR);
        assert_eq!(pixel(&low, size, 3, 3), TRACK);
    }
}
