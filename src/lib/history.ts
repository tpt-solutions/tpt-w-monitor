// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/**
 * Returns a new array with `value` appended and the oldest entries dropped so the
 * length never exceeds `capacity`. Pure, so it works cleanly with Svelte state.
 */
export function pushBounded(series: readonly number[], value: number, capacity: number): number[] {
  const keep = Math.max(0, capacity - 1);
  const next = series.length > keep ? series.slice(series.length - keep) : series.slice();
  next.push(value);
  return next;
}

/**
 * Converts a series into SVG polyline points for a `width` x `height` box. Values
 * are clamped to 0..max and mapped so that max is at the top.
 */
export function toPolylinePoints(
  series: readonly number[],
  width: number,
  height: number,
  max: number,
): string {
  if (series.length === 0) return '';
  const top = max > 0 ? max : 1;
  const step = series.length > 1 ? width / (series.length - 1) : 0;
  return series
    .map((v, i) => {
      const clamped = Math.min(Math.max(v, 0), top);
      const x = (i * step).toFixed(2);
      const y = (height - (clamped / top) * height).toFixed(2);
      return `${x},${y}`;
    })
    .join(' ');
}
