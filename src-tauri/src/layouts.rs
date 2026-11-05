// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Dashboard layouts saved as JSON files in the app data directory.
//!
//! Names are restricted to a safe character set so a name can never escape the
//! layouts folder. Content must be JSON and below a size limit. Writes go to a
//! temporary file first, then rename, so a crash never leaves a half-written layout.

use std::fs;
use std::path::{Path, PathBuf};

/// Largest layout file accepted (64 KiB is far above any realistic dashboard).
pub const MAX_LAYOUT_BYTES: usize = 64 * 1024;

/// Checks a layout name: 1 to 64 characters of `a-z`, `0-9`, `-` or `_`.
pub fn validate_name(name: &str) -> Result<(), String> {
    let ok_len = (1..=64).contains(&name.len());
    let ok_chars = name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if ok_len && ok_chars {
        Ok(())
    } else {
        Err("layout names use 1-64 characters: a-z, 0-9, '-' or '_'".into())
    }
}

fn file_for(dir: &Path, name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    Ok(dir.join(format!("{name}.json")))
}

/// Saves `content` as layout `name`, replacing any existing layout of that name.
pub fn save(dir: &Path, name: &str, content: &str) -> Result<(), String> {
    let target = file_for(dir, name)?;
    if content.len() > MAX_LAYOUT_BYTES {
        return Err(format!("layout exceeds {MAX_LAYOUT_BYTES} bytes"));
    }
    serde_json::from_str::<serde_json::Value>(content)
        .map_err(|e| format!("layout is not valid JSON: {e}"))?;

    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = target.with_extension("json.tmp");
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &target).map_err(|e| e.to_string())
}

/// Loads layout `name`, or `None` if it does not exist.
pub fn load(dir: &Path, name: &str) -> Result<Option<String>, String> {
    let path = file_for(dir, name)?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Names of saved layouts, sorted. Files that are not valid layout names are skipped.
pub fn list(dir: &Path) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let stem = file.strip_suffix(".json")?;
            validate_name(stem).ok()?;
            Some(stem.to_string())
        })
        .collect();
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tpt-monitor-layouts-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn names_cannot_escape_the_folder() {
        for bad in ["../evil", "a/b", "a\\b", "", ".hidden", "Upper", "with space", "dot.name"] {
            assert!(validate_name(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(validate_name("work-setup_2").is_ok());
        assert!(validate_name(&"x".repeat(65)).is_err());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = temp_dir("roundtrip");
        save(&dir, "main", r#"{"widgets":[]}"#).unwrap();
        assert_eq!(load(&dir, "main").unwrap().as_deref(), Some(r#"{"widgets":[]}"#));
        assert_eq!(list(&dir).unwrap(), vec!["main".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_layout_loads_as_none() {
        let dir = temp_dir("missing");
        assert_eq!(load(&dir, "nope").unwrap(), None);
        assert_eq!(list(&dir).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn invalid_json_and_oversize_content_are_refused() {
        let dir = temp_dir("refuse");
        assert!(save(&dir, "bad", "{not json").is_err());
        let big = format!("\"{}\"", "x".repeat(MAX_LAYOUT_BYTES));
        assert!(save(&dir, "big", &big).is_err());
        assert!(!dir.join("bad.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn traversal_names_never_touch_the_filesystem() {
        let dir = temp_dir("traversal");
        assert!(save(&dir, "../escape", "{}").is_err());
        assert!(load(&dir, "../escape").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
