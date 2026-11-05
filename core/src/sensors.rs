// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Sensor reading types. Access is through [`crate::SensorProvider`].

/// A single temperature sensor reading.
#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureReading {
    /// Human-readable sensor label, e.g. `CPU Package`.
    pub label: String,
    /// Temperature in degrees Celsius.
    pub celsius: f32,
}

/// A single fan speed reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanReading {
    /// Human-readable fan label.
    pub label: String,
    /// Speed in revolutions per minute.
    pub rpm: u32,
}
