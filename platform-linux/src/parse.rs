// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Parsers for the text formats of `/proc` and `/sys`. These take `&str` and touch
//! no files, so they compile and are tested on every platform.

/// Fields from `/proc/<pid>/stat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStat {
    /// Executable name (the `comm` field, without parentheses).
    pub name: String,
    /// Cumulative user + system time, in clock ticks.
    pub busy_ticks: u64,
    /// Process start time since boot, in clock ticks. Used to detect PID reuse.
    pub start_ticks: u64,
}

/// Parses `/proc/<pid>/stat`.
///
/// `comm` may itself contain spaces and parentheses, so the record is split at the
/// *last* `)`. Fields after it are numbered from `state` (field 3 in proc(5)).
pub fn parse_proc_stat(text: &str) -> Option<ProcStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = text[open + 1..close].to_string();
    let rest: Vec<&str> = text[close + 1..].split_whitespace().collect();
    // rest[0] is field 3 (state); field n is rest[n - 3].
    let utime: u64 = rest.get(11)?.parse().ok()?; // field 14
    let stime: u64 = rest.get(12)?.parse().ok()?; // field 15
    let start_ticks: u64 = rest.get(19)?.parse().ok()?; // field 22
    Some(ProcStat { name, busy_ticks: utime.saturating_add(stime), start_ticks })
}

/// Cumulative `(read, write)` bytes from `/proc/<pid>/io`, using `rchar`/`wchar`.
///
/// These count every read and write syscall, including pipes and sockets. That
/// matches what Windows `GetProcessIoCounters` reports, so the two platforms agree.
pub fn parse_proc_io(text: &str) -> Option<(u64, u64)> {
    let mut read = None;
    let mut write = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("rchar:") {
            read = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("wchar:") {
            write = v.trim().parse().ok();
        }
    }
    Some((read?, write?))
}

/// Memory from `/proc/<pid>/status`, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcStatus {
    /// `VmRSS`: resident set size.
    pub rss_bytes: u64,
    /// `RssAnon`: anonymous (private) resident memory. Zero on kernels without it.
    pub anon_bytes: u64,
    /// `VmSize`: virtual size, used as the commit-equivalent.
    pub vm_size_bytes: u64,
}

/// Parses the memory lines of `/proc/<pid>/status` (values are in kB).
pub fn parse_proc_status(text: &str) -> ProcStatus {
    let mut out = ProcStatus::default();
    for line in text.lines() {
        let kb = |v: &str| -> u64 {
            v.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0) * 1024
        };
        if let Some(v) = line.strip_prefix("VmRSS:") {
            out.rss_bytes = kb(v);
        } else if let Some(v) = line.strip_prefix("RssAnon:") {
            out.anon_bytes = kb(v);
        } else if let Some(v) = line.strip_prefix("VmSize:") {
            out.vm_size_bytes = kb(v);
        }
    }
    out
}

/// Aggregate and per-core `(total, idle)` ticks from `/proc/stat`.
///
/// Total is the sum of the first eight CPU-time fields (user through steal). Idle
/// counts `idle` and `iowait`, the same convention `top` uses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpuTimes {
    /// The aggregate `cpu` line.
    pub total: (u64, u64),
    /// One entry per `cpuN` line, in core order.
    pub per_core: Vec<(u64, u64)>,
}

fn cpu_line_ticks(fields: &[&str]) -> Option<(u64, u64)> {
    let nums: Vec<u64> = fields.iter().take(8).map(|f| f.parse().ok()).collect::<Option<_>>()?;
    if nums.len() < 4 {
        return None;
    }
    let total: u64 = nums.iter().sum();
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

/// Parses the `cpu` and `cpuN` lines of `/proc/stat`.
pub fn parse_proc_stat_cpu(text: &str) -> Option<CpuTimes> {
    let mut out = CpuTimes::default();
    let mut saw_aggregate = false;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(label) = parts.next() else { continue };
        if !label.starts_with("cpu") {
            continue;
        }
        let fields: Vec<&str> = parts.collect();
        let ticks = cpu_line_ticks(&fields)?;
        if label == "cpu" {
            out.total = ticks;
            saw_aggregate = true;
        } else if label[3..].chars().all(|c| c.is_ascii_digit()) {
            out.per_core.push(ticks);
        }
    }
    saw_aggregate.then_some(out)
}

/// `(total, available)` physical memory in bytes from `/proc/meminfo`.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        let kb = |v: &str| v.split_whitespace().next()?.parse::<u64>().ok().map(|n| n * 1024);
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = kb(v);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            available = kb(v);
        }
    }
    Some((total?, available?))
}

/// Whole-machine `(received, transmitted)` bytes from `/proc/net/dev`, excluding loopback.
///
/// Each interface line is `name: rx_bytes rx_packets ... (8 receive fields) tx_bytes ...`,
/// so the transmit byte count is the ninth value after the colon. Returns `None` when
/// no interface line parses.
pub fn parse_net_dev(text: &str) -> Option<(u64, u64)> {
    let mut total = None::<(u64, u64)>;
    for line in text.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        if name.trim() == "lo" {
            continue;
        }
        let values: Vec<u64> =
            rest.split_whitespace().map(|v| v.parse().ok()).collect::<Option<_>>()?;
        if values.len() < 9 {
            return None;
        }
        let (rx, tx) = total.unwrap_or((0, 0));
        total = Some((rx.saturating_add(values[0]), tx.saturating_add(values[8])));
    }
    total
}

/// Parses a single sysfs integer value (hwmon files end with a newline).
pub fn parse_sysfs_int(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // comm contains a space and a parenthesis to exercise the last-')' rule.
    const STAT: &str = "1234 (my (weird) app) S 1 1234 1234 0 -1 4194560 100 0 0 0 \
        250 75 0 0 20 0 3 0 98765 123456 789 18446744073709551615 1 1 0 0 0 0 0 0";

    #[test]
    fn stat_handles_parentheses_in_comm() {
        let s = parse_proc_stat(STAT).unwrap();
        assert_eq!(s.name, "my (weird) app");
        assert_eq!(s.busy_ticks, 325); // utime 250 + stime 75
        assert_eq!(s.start_ticks, 98765);
    }

    #[test]
    fn stat_rejects_truncated_record() {
        assert_eq!(parse_proc_stat("42 (x) S 1"), None);
        assert_eq!(parse_proc_stat("no parens here"), None);
    }

    #[test]
    fn io_reads_rchar_and_wchar() {
        let text =
            "rchar: 1000\nwchar: 2000\nsyscr: 5\nsyscw: 6\nread_bytes: 4096\nwrite_bytes: 0\n";
        assert_eq!(parse_proc_io(text), Some((1000, 2000)));
        assert_eq!(parse_proc_io("rchar: 1\n"), None);
    }

    #[test]
    fn status_converts_kb_to_bytes() {
        let text = "Name:\tapp\nVmSize:\t  2048 kB\nVmRSS:\t 512 kB\nRssAnon:\t 128 kB\n";
        let s = parse_proc_status(text);
        assert_eq!(s.vm_size_bytes, 2048 * 1024);
        assert_eq!(s.rss_bytes, 512 * 1024);
        assert_eq!(s.anon_bytes, 128 * 1024);
    }

    #[test]
    fn status_missing_rss_anon_is_zero() {
        let s = parse_proc_status("VmRSS:\t 10 kB\n");
        assert_eq!(s.anon_bytes, 0);
    }

    #[test]
    fn proc_stat_cpu_aggregate_and_cores() {
        let text = "cpu  100 0 50 800 50 0 0 0 0 0\n\
                    cpu0 60 0 30 400 20 0 0 0 0 0\n\
                    cpu1 40 0 20 400 30 0 0 0 0 0\n\
                    intr 12345 1 2 3\n";
        let c = parse_proc_stat_cpu(text).unwrap();
        assert_eq!(c.total, (1000, 850)); // idle 800 + iowait 50
        assert_eq!(c.per_core, vec![(510, 420), (490, 430)]);
    }

    #[test]
    fn proc_stat_cpu_requires_aggregate_line() {
        assert_eq!(parse_proc_stat_cpu("cpu0 1 2 3 4\n"), None);
    }

    #[test]
    fn meminfo_reads_total_and_available() {
        let text = "MemTotal:       16000000 kB\nMemFree:  1 kB\nMemAvailable:    4000000 kB\n";
        assert_eq!(parse_meminfo(text), Some((16_000_000 * 1024, 4_000_000 * 1024)));
        assert_eq!(parse_meminfo("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn net_dev_sums_interfaces_except_loopback() {
        let text = "Inter-|   Receive\n face |bytes packets\n    lo:  9999 1 0 0 0 0 0 0  9999 1 0 0 0 0 0 0\n  eth0: 1000 2 0 0 0 0 0 0  500 3 0 0 0 0 0 0\n wlan0: 200 1 0 0 0 0 0 0  100 1 0 0 0 0 0 0\n";
        assert_eq!(parse_net_dev(text), Some((1200, 600)));
        assert_eq!(parse_net_dev("header\nonly\n"), None);
    }

    #[test]
    fn sysfs_int_trims_newline() {
        assert_eq!(parse_sysfs_int("45000\n"), Some(45000));
        assert_eq!(parse_sysfs_int("garbage"), None);
    }
}
