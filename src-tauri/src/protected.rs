// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Storage for the user's additions to the protected-process list. Only names the core
//! accepts are ever kept, so a damaged or hand-edited file cannot weaken the built-in list.

use std::fs;
use std::path::Path;

/// Loads saved names. A missing or damaged file gives an empty list, never an error, so
/// startup is not blocked by it. Entries that are not strings are skipped.
pub fn load(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<serde_json::Value>>(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// Saves names atomically (temporary file, then rename).
pub fn save(path: &Path, names: &[String]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(names).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tpt-protected-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("protected.json")
    }

    #[test]
    fn names_round_trip() {
        let path = temp("roundtrip");
        save(&path, &["mine.exe".to_string()]).unwrap();
        assert_eq!(load(&path), vec!["mine.exe".to_string()]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn damaged_file_loads_as_empty_and_bad_entries_are_skipped() {
        let path = temp("damaged");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).is_empty());
        fs::write(&path, r#"["ok.exe", 7, null, "also.exe"]"#).unwrap();
        assert_eq!(load(&path), vec!["ok.exe".to_string(), "also.exe".to_string()]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
