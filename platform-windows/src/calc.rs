// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Pure delta arithmetic for counters sampled over time. No OS calls here, so
//! these functions are testable on every platform.

use std::time::Duration;

/// Windows FILETIME-style units: 100-nanosecond ticks.
pub const TICKS_PER_SEC: u64 = 10_000_000;

/// Converts a wall-clock duration to 100 ns ticks.
pub fn elapsed_ticks(elapsed: Duration) -> u64 {
    (elapsed.as_nanos() / 100) as u64
}

/// Process CPU as a percentage of the whole machine (all logical cores), 0.0..=100.0.
///
/// `busy_ticks` is the process's kernel + user time delta; `wall_ticks` is the
/// elapsed wall-clock time; `logical_cores` is the core count.
pub fn process_cpu_percent(busy_ticks: u64, wall_ticks: u64, logical_cores: usize) -> f32 {
    let capacity = wall_ticks.saturating_mul(logical_cores.max(1) as u64);
    if capacity == 0 {
        return 0.0;
    }
    (busy_ticks as f64 / capacity as f64 * 100.0).clamp(0.0, 100.0) as f32
}

/// Utilisation of one CPU from its idle-time and total-time deltas.
///
/// `total` is the full elapsed time on that CPU (busy plus idle); `idle` is the
/// idle portion. Returns 0.0 when no time has elapsed.
pub fn cpu_busy_percent(total_delta: u64, idle_delta: u64) -> f32 {
    if total_delta == 0 {
        return 0.0;
    }
    let idle = idle_delta.min(total_delta);
    ((total_delta - idle) as f64 / total_delta as f64 * 100.0) as f32
}

/// Bytes per second from a byte delta over `elapsed`. Returns 0 for a zero interval.
pub fn bytes_per_sec(delta_bytes: u64, elapsed: Duration) -> u64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0;
    }
    (delta_bytes as f64 / secs) as u64
}

/// `current - previous` for a monotonically increasing counter. A counter that
/// went backwards (reset or wrap) yields 0 rather than a huge bogus delta.
pub fn counter_delta(previous: u64, current: u64) -> u64 {
    current.saturating_sub(previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_cpu_scales_to_whole_machine() {
        // One full core busy for the whole interval on an 8-core machine = 12.5%.
        let wall = TICKS_PER_SEC; // 1 s
        let busy = TICKS_PER_SEC; // 1 s of CPU time
        let pct = process_cpu_percent(busy, wall, 8);
        assert!((pct - 12.5).abs() < 0.001, "got {pct}");
    }

    #[test]
    fn process_cpu_is_clamped_and_zero_safe() {
        assert_eq!(process_cpu_percent(10, 0, 4), 0.0);
        assert_eq!(process_cpu_percent(u64::MAX, 1, 1), 100.0);
    }

    #[test]
    fn cpu_busy_handles_idle_larger_than_total() {
        assert_eq!(cpu_busy_percent(100, 100), 0.0);
        assert_eq!(cpu_busy_percent(100, 0), 100.0);
        assert_eq!(cpu_busy_percent(100, 250), 0.0);
        assert_eq!(cpu_busy_percent(0, 0), 0.0);
    }

    #[test]
    fn rates_handle_zero_interval() {
        assert_eq!(bytes_per_sec(1000, Duration::ZERO), 0);
        assert_eq!(bytes_per_sec(2000, Duration::from_secs(2)), 1000);
    }

    #[test]
    fn counter_reset_is_not_a_huge_delta() {
        assert_eq!(counter_delta(10, 25), 15);
        assert_eq!(counter_delta(25, 10), 0);
    }

    #[test]
    fn elapsed_ticks_converts_units() {
        assert_eq!(elapsed_ticks(Duration::from_secs(1)), TICKS_PER_SEC);
    }
}
