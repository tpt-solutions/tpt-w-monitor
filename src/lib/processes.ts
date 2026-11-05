// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import type { ProcessDto } from './types';

export type SortKey = 'name' | 'pid' | 'cpu' | 'memory' | 'disk';

/** Case-insensitive substring filter on name or PID. */
export function filterProcesses(list: readonly ProcessDto[], query: string): ProcessDto[] {
  const q = query.trim().toLowerCase();
  if (q === '') return list.slice();
  return list.filter((p) => p.name.toLowerCase().includes(q) || String(p.pid).includes(q));
}

function diskTotal(p: ProcessDto): number {
  return (p.diskReadBytesPerSec ?? 0) + (p.diskWriteBytesPerSec ?? 0);
}

/**
 * Sorts a copy of the list by `key`, in either direction. Ties always fall back to
 * ascending PID, so rows with equal values do not reshuffle between samples.
 */
export function sortProcesses(
  list: readonly ProcessDto[],
  key: SortKey,
  descending: boolean,
): ProcessDto[] {
  const sign = descending ? -1 : 1;
  const primary: Record<SortKey, (a: ProcessDto, b: ProcessDto) => number> = {
    name: (a, b) => a.name.localeCompare(b.name),
    pid: (a, b) => a.pid - b.pid,
    cpu: (a, b) => a.cpuPercent - b.cpuPercent,
    memory: (a, b) => a.workingSetBytes - b.workingSetBytes,
    disk: (a, b) => diskTotal(a) - diskTotal(b),
  };
  return list.slice().sort((a, b) => sign * primary[key](a, b) || a.pid - b.pid);
}
