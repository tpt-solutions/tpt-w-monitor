// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Desktop overlay bookkeeping: which widgets are popped out, where they sit, and
//! how they look. Pure data and file handling, so it is testable without a window.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Widget kinds that may be popped out. Must match `WIDGET_KINDS` in src/lib/layout.ts.
pub const OVERLAY_KINDS: [&str; 8] =
    ["cpu", "memory", "processes", "sensors", "network", "disk", "gpu", "suggestions"];

/// Most overlays open at once. Each is a separate webview, so this bounds memory use.
pub const MAX_OVERLAYS: usize = 8;

/// Overlay window size bounds, in logical pixels.
pub const MIN_SIZE: u32 = 120;
/// See [`MIN_SIZE`].
pub const MAX_SIZE: u32 = 2400;
/// Size used when an overlay is first opened.
pub const DEFAULT_SIZE: (u32, u32) = (320, 240);

/// Saved state of one overlay window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayConfig {
    /// Stable identifier, the widget id on the dashboard, e.g. `cpu-1`.
    pub id: String,
    /// Widget kind shown in the overlay.
    pub kind: String,
    /// Screen position, logical pixels (the same units Tauri uses for window placement).
    pub x: i32,
    /// Screen position, logical pixels.
    pub y: i32,
    /// Width, logical pixels.
    pub width: u32,
    /// Height, logical pixels.
    pub height: u32,
    /// Window opacity applied by the overlay page, 0.2..=1.0.
    pub opacity: f32,
    /// Whether mouse input passes through the overlay.
    pub click_through: bool,
}

/// Checks an overlay id: `<kind>-<n>` with a known kind and a small number. Ids become
/// window labels, so this also keeps labels to a safe character set.
pub fn validate_id(id: &str) -> Result<(), String> {
    let (kind, number) =
        id.rsplit_once('-').ok_or_else(|| "overlay id must look like cpu-1".to_string())?;
    if !OVERLAY_KINDS.contains(&kind) {
        return Err(format!("unknown widget kind '{kind}'"));
    }
    let ok_number =
        !number.is_empty() && number.len() <= 3 && number.bytes().all(|b| b.is_ascii_digit());
    if !ok_number {
        return Err("overlay id must end with a number of up to three digits".into());
    }
    Ok(())
}

/// Checks that a kind is poppable.
pub fn validate_kind(kind: &str) -> Result<(), String> {
    if OVERLAY_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(format!("unknown widget kind '{kind}'"))
    }
}

/// Clamps a size into the allowed range.
pub fn clamp_size(width: u32, height: u32) -> (u32, u32) {
    (width.clamp(MIN_SIZE, MAX_SIZE), height.clamp(MIN_SIZE, MAX_SIZE))
}

/// Clamps opacity into 0.2..=1.0. A fully transparent overlay could never be found again.
pub fn clamp_opacity(opacity: f32) -> f32 {
    if opacity.is_nan() {
        return 1.0;
    }
    opacity.clamp(0.2, 1.0)
}

/// The set of overlays, persisted as JSON in the app data folder.
#[derive(Debug)]
pub struct OverlayStore {
    path: PathBuf,
    items: BTreeMap<String, OverlayConfig>,
}

impl OverlayStore {
    /// Loads the store from `path`. A missing or damaged file gives an empty store, so a
    /// bad file never stops the app from starting.
    pub fn load(path: PathBuf) -> Self {
        let items = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<OverlayConfig>>(&text).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|c| validate_id(&c.id).is_ok() && validate_kind(&c.kind).is_ok())
            .take(MAX_OVERLAYS)
            .map(|c| (c.id.clone(), c))
            .collect();
        Self { path, items }
    }

    /// Writes the store to disk atomically (temporary file, then rename).
    pub fn save(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let list: Vec<&OverlayConfig> = self.items.values().collect();
        let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, text).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    /// Adds a new overlay. Fails when the id is taken or the limit is reached.
    pub fn insert(&mut self, config: OverlayConfig) -> Result<(), String> {
        validate_id(&config.id)?;
        validate_kind(&config.kind)?;
        if self.items.contains_key(&config.id) {
            return Err(format!("overlay {} is already open", config.id));
        }
        if self.items.len() >= MAX_OVERLAYS {
            return Err(format!("at most {MAX_OVERLAYS} overlays can be open"));
        }
        self.items.insert(config.id.clone(), config);
        Ok(())
    }

    /// Removes an overlay, returning its last state.
    pub fn remove(&mut self, id: &str) -> Option<OverlayConfig> {
        self.items.remove(id)
    }

    /// Mutable access to one overlay.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut OverlayConfig> {
        self.items.get_mut(id)
    }

    /// All overlays, in id order.
    pub fn list(&self) -> Vec<OverlayConfig> {
        self.items.values().cloned().collect()
    }

    /// Path the store saves to.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(id: &str, kind: &str) -> OverlayConfig {
        OverlayConfig {
            id: id.into(),
            kind: kind.into(),
            x: 10,
            y: 20,
            width: 300,
            height: 200,
            opacity: 0.8,
            click_through: false,
        }
    }

    fn temp_file(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tpt-overlays-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("overlays.json")
    }

    #[test]
    fn ids_must_be_known_kind_and_number() {
        assert!(validate_id("cpu-1").is_ok());
        assert!(validate_id("processes-42").is_ok());
        for bad in
            ["cpu", "cpu-", "cpu-1234", "shell-1", "cpu-1/../x", "CPU-1", "cpu-a", "../cpu-1"]
        {
            assert!(validate_id(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn kinds_are_checked_against_the_allow_list() {
        assert!(validate_kind("gpu").is_ok());
        assert!(validate_kind("terminal").is_err());
    }

    #[test]
    fn size_and_opacity_are_clamped() {
        assert_eq!(clamp_size(1, 99_999), (MIN_SIZE, MAX_SIZE));
        assert_eq!(clamp_opacity(0.0), 0.2);
        assert_eq!(clamp_opacity(3.0), 1.0);
        assert_eq!(clamp_opacity(f32::NAN), 1.0);
    }

    #[test]
    fn insert_enforces_uniqueness_and_limit() {
        let mut store = OverlayStore::load(temp_file("limit"));
        store.insert(config("cpu-1", "cpu")).unwrap();
        assert!(store.insert(config("cpu-1", "cpu")).is_err());
        for n in 2..=MAX_OVERLAYS {
            store.insert(config(&format!("memory-{n}"), "memory")).unwrap();
        }
        assert!(store.insert(config("gpu-99", "gpu")).is_err());
    }

    #[test]
    fn save_and_reload_round_trips() {
        let path = temp_file("roundtrip");
        let mut store = OverlayStore::load(path.clone());
        store.insert(config("cpu-1", "cpu")).unwrap();
        store.save().unwrap();

        let reloaded = OverlayStore::load(path.clone());
        assert_eq!(reloaded.list(), vec![config("cpu-1", "cpu")]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn damaged_file_loads_as_empty_and_bad_entries_are_dropped() {
        let path = temp_file("damaged");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert!(OverlayStore::load(path.clone()).list().is_empty());

        let mixed = format!(
            "[{}, {}]",
            serde_json::to_string(&config("cpu-1", "cpu")).unwrap(),
            serde_json::to_string(&config("evil-1", "terminal")).unwrap()
        );
        fs::write(&path, mixed).unwrap();
        let ids: Vec<String> =
            OverlayStore::load(path.clone()).list().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, vec!["cpu-1".to_string()]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
