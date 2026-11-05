// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Linux backend for tpt-monitor.
//!
//! [`parse`] holds the `/proc` and `/sys` text parsers and compiles on every
//! platform so it can be unit-tested anywhere. The monitor and sensor
//! implementations are Linux-only.
//!
//! Not yet provided (see todo.md Phases 3-4): per-process network attribution via
//! `/proc/net` inode matching, DRM `fdinfo` GPU stats, per-drive I/O, window
//! detection, and the root-launch / polkit path.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod parse;

#[cfg(target_os = "linux")]
mod backend;

#[cfg(target_os = "linux")]
pub use backend::{LinuxMonitor, LinuxSensors, is_elevated};
