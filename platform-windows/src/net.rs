// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Per-process TCP traffic: parsing the IP Helper owner table and turning cumulative
//! per-connection byte counters into per-process rates. No OS calls here, so this is
//! testable on every platform.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// `MIB_TCP_STATE_ESTAB`: only established connections carry ESTATS byte counters.
pub const STATE_ESTABLISHED: u32 = 5;

/// Size of one `MIB_TCPROW_OWNER_PID` record in the table buffer.
pub const OWNER_ROW_SIZE: usize = 24;

/// One TCP connection from the owner-PID table. Ports are host order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TcpRow {
    /// MIB state, e.g. [`STATE_ESTABLISHED`].
    pub state: u32,
    /// Local IPv4 address, network byte order as IP Helper stores it.
    pub local_addr: u32,
    /// Local port, host order.
    pub local_port: u16,
    /// Remote IPv4 address, network byte order as IP Helper stores it.
    pub remote_addr: u32,
    /// Remote port, host order.
    pub remote_port: u16,
    /// Owning process.
    pub pid: u32,
}

/// Parses the buffer returned by `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)`.
///
/// The layout is a 4-byte entry count followed by `count` owner rows. Rows whose
/// declared count would run past the buffer are dropped rather than read.
pub fn parse_owner_table(buf: &[u8]) -> Vec<TcpRow> {
    if buf.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let available = (buf.len() - 4) / OWNER_ROW_SIZE;
    let rows = count.min(available);
    (0..rows)
        .map(|i| {
            let base = 4 + i * OWNER_ROW_SIZE;
            let field = |k: usize| {
                let o = base + k * 4;
                u32::from_ne_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
            };
            TcpRow {
                state: field(0),
                local_addr: field(1),
                // Ports are stored in the low 16 bits, in network byte order.
                local_port: u16::from_be(field(2) as u16),
                remote_addr: field(3),
                remote_port: u16::from_be(field(4) as u16),
                pid: field(5),
            }
        })
        .collect()
}

/// One connection's cumulative counters at a sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnSample {
    /// Owning process.
    pub pid: u32,
    /// Connection identity, stable while the connection lives.
    pub key: TcpRow,
    /// Cumulative bytes received, or `None` if the counter could not be read.
    pub counters: Option<(u64, u64)>,
}

/// Per-process rates over one interval, bytes per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetRates {
    /// Bytes sent per second.
    pub up: u64,
    /// Bytes received per second.
    pub down: u64,
}

/// Turns cumulative per-connection counters into per-process rates.
///
/// A connection seen for the first time, or whose counters could not be read, adds
/// nothing for that interval. Counters that move backwards are treated as zero, so a
/// reused connection cannot produce a huge spike.
#[derive(Debug, Default)]
pub struct NetTracker {
    previous: HashMap<TcpRow, (u64, u64)>,
}

impl NetTracker {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes one sample and returns the per-process rates for the interval since
    /// the last call. Connections missing from `samples` are forgotten.
    pub fn observe(&mut self, samples: &[ConnSample], elapsed: Duration) -> HashMap<u32, NetRates> {
        let live: HashSet<TcpRow> = samples.iter().map(|s| s.key).collect();
        self.previous.retain(|key, _| live.contains(key));

        let mut totals: HashMap<u32, NetRates> = HashMap::new();
        let secs = elapsed.as_secs_f64();
        for sample in samples {
            let Some((rx, tx)) = sample.counters else {
                self.previous.remove(&sample.key);
                continue;
            };
            if let Some(&(prev_rx, prev_tx)) = self.previous.get(&sample.key) {
                if secs > 0.0 {
                    let entry = totals.entry(sample.pid).or_default();
                    entry.down += ((rx.saturating_sub(prev_rx)) as f64 / secs) as u64;
                    entry.up += ((tx.saturating_sub(prev_tx)) as f64 / secs) as u64;
                }
            }
            self.previous.insert(sample.key, (rx, tx));
        }
        totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, local_port: u16) -> TcpRow {
        TcpRow {
            state: STATE_ESTABLISHED,
            local_addr: 0x0100_007f,
            local_port,
            remote_addr: 0x0100_007f,
            remote_port: 443,
            pid,
        }
    }

    fn owner_buffer(rows: &[(u32, u32, u16, u32, u16, u32)]) -> Vec<u8> {
        let mut buf = (rows.len() as u32).to_ne_bytes().to_vec();
        for &(state, la, lp, ra, rp, pid) in rows {
            buf.extend_from_slice(&state.to_ne_bytes());
            buf.extend_from_slice(&la.to_ne_bytes());
            buf.extend_from_slice(&u32::from(lp.to_be()).to_ne_bytes());
            buf.extend_from_slice(&ra.to_ne_bytes());
            buf.extend_from_slice(&u32::from(rp.to_be()).to_ne_bytes());
            buf.extend_from_slice(&pid.to_ne_bytes());
        }
        buf
    }

    #[test]
    fn owner_table_parses_rows_and_converts_ports() {
        let buf = owner_buffer(&[(5, 0x0100_007f, 51234, 0x0100_007f, 443, 777)]);
        let rows = parse_owner_table(&buf);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pid, 777);
        assert_eq!(rows[0].local_port, 51234);
        assert_eq!(rows[0].remote_port, 443);
        assert_eq!(rows[0].state, STATE_ESTABLISHED);
    }

    #[test]
    fn truncated_table_is_clamped_not_read_past_the_end() {
        let mut buf = owner_buffer(&[(5, 1, 1, 1, 1, 1), (5, 2, 2, 2, 2, 2)]);
        buf.truncate(buf.len() - 10);
        assert_eq!(parse_owner_table(&buf).len(), 1);
        assert!(parse_owner_table(&[1, 0]).is_empty());
        assert!(parse_owner_table(&[]).is_empty());
    }

    #[test]
    fn rates_come_from_counter_deltas_per_process() {
        let mut t = NetTracker::new();
        let a = row(10, 1000);
        let b = row(10, 1001);
        let first = [
            ConnSample { pid: 10, key: a, counters: Some((1_000, 500)) },
            ConnSample { pid: 10, key: b, counters: Some((0, 0)) },
        ];
        assert!(t.observe(&first, Duration::from_secs(1)).is_empty(), "first sample has no delta");

        let second = [
            ConnSample { pid: 10, key: a, counters: Some((3_000, 1_500)) },
            ConnSample { pid: 10, key: b, counters: Some((1_000, 0)) },
        ];
        let rates = t.observe(&second, Duration::from_secs(2));
        // a: +2000 down, +1000 up; b: +1000 down; over 2 s.
        assert_eq!(rates[&10], NetRates { up: 500, down: 1_500 });
    }

    #[test]
    fn unreadable_counters_contribute_nothing() {
        let mut t = NetTracker::new();
        let a = row(20, 2000);
        t.observe(
            &[ConnSample { pid: 20, key: a, counters: Some((10, 10)) }],
            Duration::from_secs(1),
        );
        let rates =
            t.observe(&[ConnSample { pid: 20, key: a, counters: None }], Duration::from_secs(1));
        assert!(!rates.contains_key(&20));
    }

    #[test]
    fn counters_that_go_backwards_do_not_spike() {
        let mut t = NetTracker::new();
        let a = row(30, 3000);
        t.observe(
            &[ConnSample { pid: 30, key: a, counters: Some((9_000, 9_000)) }],
            Duration::from_secs(1),
        );
        let rates = t.observe(
            &[ConnSample { pid: 30, key: a, counters: Some((100, 100)) }],
            Duration::from_secs(1),
        );
        assert_eq!(rates[&30], NetRates::default());
    }

    #[test]
    fn closed_connections_are_forgotten() {
        let mut t = NetTracker::new();
        let a = row(40, 4000);
        t.observe(
            &[ConnSample { pid: 40, key: a, counters: Some((1, 1)) }],
            Duration::from_secs(1),
        );
        t.observe(&[], Duration::from_secs(1));
        // Reappearing later starts a fresh baseline, not a delta from the old connection.
        let rates = t.observe(
            &[ConnSample { pid: 40, key: a, counters: Some((5_000, 5_000)) }],
            Duration::from_secs(1),
        );
        assert!(rates.is_empty());
    }
}

/// `IF_TYPE_SOFTWARE_LOOPBACK`: loopback traffic does not leave the machine.
pub const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;

/// Sums cumulative `(interface type, in octets, out octets)` for every interface except
/// software loopback.
pub fn sum_interface_octets(rows: &[(u32, u64, u64)]) -> (u64, u64) {
    rows.iter().filter(|(kind, _, _)| *kind != IF_TYPE_SOFTWARE_LOOPBACK).fold(
        (0u64, 0u64),
        |(total_in, total_out), (_, i, o)| {
            (total_in.saturating_add(*i), total_out.saturating_add(*o))
        },
    )
}

/// Rates between two cumulative `(in, out)` totals taken `elapsed` apart. Counters that
/// move backwards count as zero.
pub fn rate_between(previous: (u64, u64), now: (u64, u64), elapsed: Duration) -> NetRates {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return NetRates::default();
    }
    NetRates {
        up: ((now.1.saturating_sub(previous.1)) as f64 / secs) as u64,
        down: ((now.0.saturating_sub(previous.0)) as f64 / secs) as u64,
    }
}

#[cfg(test)]
mod interface_tests {
    use super::*;

    #[test]
    fn loopback_is_excluded_from_totals() {
        let rows = [(6, 100, 50), (IF_TYPE_SOFTWARE_LOOPBACK, 9_999, 9_999), (71, 25, 5)];
        assert_eq!(sum_interface_octets(&rows), (125, 55));
    }

    #[test]
    fn rates_come_from_total_deltas() {
        let r = rate_between((1_000, 500), (3_000, 1_500), Duration::from_secs(2));
        assert_eq!(r, NetRates { up: 500, down: 1_000 });
        assert_eq!(rate_between((5, 5), (1, 1), Duration::from_secs(1)), NetRates::default());
        assert_eq!(rate_between((0, 0), (9, 9), Duration::ZERO), NetRates::default());
    }
}
