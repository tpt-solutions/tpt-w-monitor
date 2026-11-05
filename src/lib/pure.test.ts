// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from 'vitest';
import { formatBytes, formatPercent, formatRate } from './format';
import { pushBounded, toPolylinePoints } from './history';
import {
  GRID_COLUMNS,
  defaultLayout,
  nextWidgetId,
  parseLayout,
  serializeLayout,
} from './layout';
import { filterProcesses, sortProcesses } from './processes';
import { applyTheme, loadTheme, saveTheme } from './theme';
import type { ProcessDto } from './types';

function proc(pid: number, name: string, cpu: number, mem = 0, read: number | null = null): ProcessDto {
  return {
    pid,
    name,
    cpuPercent: cpu,
    workingSetBytes: mem,
    privateBytes: 0,
    commitBytes: 0,
    gpuEnginePercent: null,
    vramBytes: null,
    diskReadBytesPerSec: read,
    diskWriteBytesPerSec: null,
    netUpBytesPerSec: null,
    netDownBytesPerSec: null,
    hasVisibleWindow: false,
  };
}

describe('format', () => {
  it('shows an em dash for unavailable values', () => {
    expect(formatBytes(null)).toBe('—');
    expect(formatRate(undefined)).toBe('—');
    expect(formatPercent(null)).toBe('—');
    expect(formatPercent(Number.NaN)).toBe('—');
  });

  it('uses binary units and sensible precision', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1536)).toBe('1.5 KiB');
    expect(formatBytes(16 * 1024 ** 3)).toBe('16 GiB');
    expect(formatRate(2048)).toBe('2.0 KiB/s');
    expect(formatPercent(12.345)).toBe('12.3%');
  });
});

describe('history', () => {
  it('keeps only the newest values within capacity and does not mutate input', () => {
    const original = [1, 2, 3];
    const next = pushBounded(original, 4, 3);
    expect(next).toEqual([2, 3, 4]);
    expect(original).toEqual([1, 2, 3]);
    expect(pushBounded([], 9, 5)).toEqual([9]);
  });

  it('maps values into the SVG box with max at the top and clamps out-of-range values', () => {
    expect(toPolylinePoints([], 100, 50, 100)).toBe('');
    expect(toPolylinePoints([0, 100], 100, 50, 100)).toBe('0.00,50.00 100.00,0.00');
    expect(toPolylinePoints([250], 100, 50, 100)).toBe('0.00,0.00');
  });
});

describe('layout', () => {
  it('default layout is valid and round-trips through JSON', () => {
    const doc = defaultLayout();
    const text = serializeLayout(doc.widgets);
    expect(parseLayout(text)).toEqual(doc);
  });

  it('rejects damaged or hostile layout files', () => {
    expect(parseLayout('not json')).toBeNull();
    expect(parseLayout('[]')).toBeNull();
    expect(parseLayout(JSON.stringify({ version: 2, widgets: [] }))).toBeNull();
    expect(
      parseLayout(JSON.stringify({ version: 1, widgets: [{ id: 'x-1', kind: 'shell', x: 0, y: 0, w: 2, h: 2 }] })),
    ).toBeNull();
    expect(
      parseLayout(JSON.stringify({ version: 1, widgets: [{ id: 'cpu-1', kind: 'cpu', x: 11, y: 0, w: 4, h: 2 }] })),
    ).toBeNull();
    expect(
      parseLayout(
        JSON.stringify({
          version: 1,
          widgets: [
            { id: 'cpu-1', kind: 'cpu', x: 0, y: 0, w: 2, h: 2 },
            { id: 'cpu-1', kind: 'cpu', x: 2, y: 0, w: 2, h: 2 },
          ],
        }),
      ),
    ).toBeNull();
    expect(
      parseLayout(JSON.stringify({ version: 1, widgets: [{ id: 'cpu-1', kind: 'cpu', x: 0.5, y: 0, w: 2, h: 2 }] })),
    ).toBeNull();
  });

  it('never accepts a placement wider than the grid', () => {
    const text = JSON.stringify({
      version: 1,
      widgets: [{ id: 'cpu-1', kind: 'cpu', x: 0, y: 0, w: GRID_COLUMNS + 1, h: 2 }],
    });
    expect(parseLayout(text)).toBeNull();
  });

  it('picks the next free id for a kind', () => {
    expect(nextWidgetId('cpu', [])).toBe('cpu-1');
    expect(nextWidgetId('cpu', ['cpu-1', 'cpu-3'])).toBe('cpu-2');
  });
});

describe('processes', () => {
  const list = [proc(10, 'zeta.exe', 5), proc(2, 'Alpha.exe', 40, 100), proc(7, 'beta.exe', 40, 50, 9)];

  it('filters by name or PID, case-insensitively', () => {
    expect(filterProcesses(list, 'ALPHA').map((p) => p.pid)).toEqual([2]);
    expect(filterProcesses(list, '10').map((p) => p.pid)).toEqual([10]);
    expect(filterProcesses(list, '  ').length).toBe(3);
  });

  it('sorts by CPU descending with a stable PID tie-break, without mutating the input', () => {
    const sorted = sortProcesses(list, 'cpu', true).map((p) => p.pid);
    expect(sorted).toEqual([2, 7, 10]);
    expect(list.map((p) => p.pid)).toEqual([10, 2, 7]);
  });

  it('sorts names ascending and treats missing disk data as zero', () => {
    expect(sortProcesses(list, 'name', false).map((p) => p.name)).toEqual([
      'Alpha.exe',
      'beta.exe',
      'zeta.exe',
    ]);
    expect(sortProcesses(list, 'disk', true)[0].pid).toBe(7);
  });
});

describe('theme', () => {
  it('falls back to system when storage is blocked or holds junk', () => {
    const original = globalThis.localStorage;
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      get() {
        throw new Error('blocked');
      },
    });
    try {
      expect(loadTheme()).toBe('system');
      expect(() => saveTheme('dark')).not.toThrow();
    } finally {
      Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: original });
    }
  });

  it('applies and clears data-theme on a root element', () => {
    const root = { dataset: { theme: 'light' } } as unknown as HTMLElement;
    applyTheme('dark', root);
    expect(root.dataset.theme).toBe('dark');
    applyTheme('system', root);
    expect(root.dataset.theme).toBeUndefined();
  });
});
