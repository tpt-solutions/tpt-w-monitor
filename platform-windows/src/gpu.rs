// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Turning PDH GPU counter instances into per-process values. Pure string and
//! arithmetic work, so it is testable on every platform; the PDH calls live in `sys.rs`.

use std::collections::HashMap;

/// Extracts the process id from a `GPU Engine` or `GPU Process Memory` instance name.
///
/// Instance names look like `pid_1234_luid_0x00000000_0x0000C9E5_phys_0_eng_0_engtype_3D`.
/// Anything else (system-wide or unrecognised instances) yields `None`.
pub fn pid_from_instance(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("pid_")?;
    let digits = rest.split('_').next()?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Reduces per-engine utilisation rows to one percentage per process.
///
/// A process's GPU load is its busiest engine (3D, copy, video and so on), which is the
/// figure Task Manager shows. Values are clamped to 0..=100.
pub fn engine_percent_by_pid(items: &[(String, f64)]) -> HashMap<u32, f32> {
    let mut out: HashMap<u32, f32> = HashMap::new();
    for (name, value) in items {
        let Some(pid) = pid_from_instance(name) else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        let v = value.clamp(0.0, 100.0) as f32;
        let slot = out.entry(pid).or_insert(0.0);
        if v > *slot {
            *slot = v;
        }
    }
    out
}

/// Sums dedicated GPU memory per process. Instances from several adapters add up.
pub fn memory_by_pid(items: &[(String, f64)]) -> HashMap<u32, u64> {
    let mut out: HashMap<u32, u64> = HashMap::new();
    for (name, value) in items {
        let Some(pid) = pid_from_instance(name) else {
            continue;
        };
        if !value.is_finite() || *value < 0.0 {
            continue;
        }
        let entry = out.entry(pid).or_insert(0);
        *entry = entry.saturating_add(*value as u64);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_yield_their_pid() {
        assert_eq!(
            pid_from_instance("pid_1234_luid_0x00000000_0x0000C9E5_phys_0_eng_0_engtype_3D"),
            Some(1234)
        );
        assert_eq!(pid_from_instance("pid_42_luid_0x0_0x0_phys_0"), Some(42));
    }

    #[test]
    fn unrecognised_instances_are_ignored() {
        assert_eq!(pid_from_instance("_Total"), None);
        assert_eq!(pid_from_instance("pid__luid"), None);
        assert_eq!(pid_from_instance("pid_abc_luid"), None);
        assert_eq!(pid_from_instance("pid_"), None);
        assert_eq!(pid_from_instance("luid_pid_1"), None);
    }

    #[test]
    fn engines_take_the_busiest_value_per_process() {
        let items = vec![
            ("pid_7_luid_a_eng_0_engtype_3D".to_string(), 30.0),
            ("pid_7_luid_a_eng_1_engtype_Copy".to_string(), 55.0),
            ("pid_8_luid_a_eng_0_engtype_3D".to_string(), 120.0),
            ("_Total".to_string(), 99.0),
            ("pid_9_luid_a_eng_0".to_string(), f64::NAN),
        ];
        let by_pid = engine_percent_by_pid(&items);
        assert_eq!(by_pid.get(&7), Some(&55.0));
        assert_eq!(by_pid.get(&8), Some(&100.0), "clamped to 100");
        assert!(!by_pid.contains_key(&9), "NaN is skipped");
        assert_eq!(by_pid.len(), 2);
    }

    #[test]
    fn memory_adds_across_adapters_and_skips_negatives() {
        let items = vec![
            ("pid_5_luid_a_phys_0".to_string(), 100.0),
            ("pid_5_luid_b_phys_0".to_string(), 250.0),
            ("pid_6_luid_a_phys_0".to_string(), -1.0),
        ];
        let by_pid = memory_by_pid(&items);
        assert_eq!(by_pid.get(&5), Some(&350));
        assert!(!by_pid.contains_key(&6));
    }
}
