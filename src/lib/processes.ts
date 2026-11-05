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

/** Processes that share an executable name, shown as one row. */
export interface ProcessGroup {
  name: string;
  pids: number[];
  cpuPercent: number;
  workingSetBytes: number;
  diskBytesPerSec: number | null;
  netBytesPerSec: number | null;
  gpuEnginePercent: number | null;
  vramBytes: number | null;
}

/** Sum of the values that are present; null only when every value is null. */
function sumOrNull(values: (number | null)[]): number | null {
  const present = values.filter((v): v is number => v !== null);
  return present.length === 0 ? null : present.reduce((a, b) => a + b, 0);
}

/**
 * Groups processes by executable name (case-insensitive) and totals their metrics. Each
 * group's PIDs are listed in ascending order. Groups come back busiest first.
 */
export function groupByName(list: readonly ProcessDto[]): ProcessGroup[] {
  const groups = new Map<string, ProcessDto[]>();
  for (const p of list) {
    const key = p.name.toLowerCase();
    const bucket = groups.get(key);
    if (bucket) bucket.push(p);
    else groups.set(key, [p]);
  }
  const out: ProcessGroup[] = [];
  for (const members of groups.values()) {
    const pids = members.map((m) => m.pid).sort((a, b) => a - b);
    out.push({
      name: members[0].name,
      pids,
      cpuPercent: members.reduce((sum, m) => sum + m.cpuPercent, 0),
      workingSetBytes: members.reduce((sum, m) => sum + m.workingSetBytes, 0),
      diskBytesPerSec: sumOrNull(
        members.map((m) =>
          m.diskReadBytesPerSec === null && m.diskWriteBytesPerSec === null
            ? null
            : (m.diskReadBytesPerSec ?? 0) + (m.diskWriteBytesPerSec ?? 0),
        ),
      ),
      netBytesPerSec: sumOrNull(
        members.map((m) =>
          m.netUpBytesPerSec === null && m.netDownBytesPerSec === null
            ? null
            : (m.netUpBytesPerSec ?? 0) + (m.netDownBytesPerSec ?? 0),
        ),
      ),
      gpuEnginePercent: sumOrNull(members.map((m) => m.gpuEnginePercent)),
      vramBytes: sumOrNull(members.map((m) => m.vramBytes)),
    });
  }
  return out.sort((a, b) => b.cpuPercent - a.cpuPercent || a.name.localeCompare(b.name));
}
