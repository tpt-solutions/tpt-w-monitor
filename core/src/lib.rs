// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Core of tpt-monitor: the platform-independent metrics model, the traits every
//! backend implements, the process-action gate, and the sampling engine.
//!
//! Platform backends (`platform-windows`, `platform-linux`) implement
//! [`SystemMonitor`] and [`SensorProvider`]; everything else in the app talks to
//! these traits only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod action;
pub mod error;
pub mod history;
pub mod mock;
pub mod model;
pub mod sampler;
pub mod scheduler;
pub mod sensors;
pub mod suggestion;
pub mod traits;

pub use action::{Priority, ProcessAction, execute_action};
pub use error::{MonitorError, Result};
pub use history::RingBuffer;
pub use model::{AdapterMetrics, DriveMetrics, MemoryUsage, Pid, ProcessMetrics, SystemMetrics};
pub use sampler::{Sampler, Snapshot};
pub use scheduler::{SamplingScheduler, ViewState};
pub use sensors::{FanReading, TemperatureReading};
pub use suggestion::{CriticalList, Suggestion, SuggestionConfig, SuggestionEngine};
pub use traits::{SensorProvider, SystemMonitor};
