// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Reactive state shared by every widget. Frames arrive from the backend as events;
// `ingest` stores the latest frame and extends the short-window graph history.

import { pushBounded } from './history';
import type { FrameDto } from './types';

/** Samples kept for the CPU and memory graphs (about two minutes at the base rate). */
export const GRAPH_SAMPLES = 120;

/** Samples kept per process for the table sparklines. */
export const PROCESS_SAMPLES = 60;

class Live {
  frame = $state<FrameDto | null>(null);
  cpu = $state<number[]>([]);
  memory = $state<number[]>([]);
  /** CPU history per PID, for the process table sparklines. */
  processHistory = $state<Record<number, number[]>>({});
  notice = $state<string | null>(null);
}

export const live = new Live();

export function ingest(frame: FrameDto): void {
  const s = frame.system;
  const memPercent = s.memoryTotalBytes > 0 ? (s.memoryUsedBytes / s.memoryTotalBytes) * 100 : 0;
  live.frame = frame;
  live.cpu = pushBounded(live.cpu, s.cpuTotalPercent, GRAPH_SAMPLES);
  live.memory = pushBounded(live.memory, memPercent, GRAPH_SAMPLES);

  // Keep history only for processes still present, so exited PIDs do not accumulate.
  const next: Record<number, number[]> = {};
  for (const p of frame.processes) {
    next[p.pid] = pushBounded(live.processHistory[p.pid] ?? [], p.cpuPercent, PROCESS_SAMPLES);
  }
  live.processHistory = next;
}

let noticeTimer: ReturnType<typeof setTimeout> | undefined;

/** Shows a message in the banner for a few seconds. */
export function notify(message: string): void {
  live.notice = message;
  if (noticeTimer !== undefined) clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => {
    live.notice = null;
  }, 6000);
}
