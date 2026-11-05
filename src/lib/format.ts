// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/** Human-readable byte count using binary units. `null` shows an em dash. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes)) return '—';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let value = Math.max(0, bytes);
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  const digits = i === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[i]}`;
}

/** Bytes per second. `null` shows an em dash. */
export function formatRate(bytesPerSec: number | null | undefined): string {
  return bytesPerSec == null ? '—' : `${formatBytes(bytesPerSec)}/s`;
}

/** Percentage with one decimal. `null` shows an em dash. */
export function formatPercent(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '—';
  return `${value.toFixed(1)}%`;
}

/** Revolutions per minute as a whole number. */
export function formatRpm(rpm: number): string {
  return `${Math.round(rpm)} RPM`;
}

/** Degrees Celsius with no decimals. */
export function formatCelsius(celsius: number): string {
  return `${Math.round(celsius)} °C`;
}
