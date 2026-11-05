// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Dashboard layout model: which widgets exist and where. Saved as JSON.

export const GRID_COLUMNS = 12;
export const MAX_WIDGETS = 40;
const MAX_ROWS = 60;

export const WIDGET_KINDS = [
  'cpu',
  'memory',
  'processes',
  'sensors',
  'network',
  'disk',
  'gpu',
  'suggestions',
] as const;

export type WidgetKind = (typeof WIDGET_KINDS)[number];

export interface WidgetInfo {
  title: string;
  w: number;
  h: number;
  minW: number;
  minH: number;
}

export const WIDGET_INFO: Record<WidgetKind, WidgetInfo> = {
  cpu: { title: 'CPU', w: 4, h: 3, minW: 2, minH: 2 },
  memory: { title: 'Memory', w: 4, h: 3, minW: 2, minH: 2 },
  processes: { title: 'Processes', w: 12, h: 7, minW: 6, minH: 4 },
  sensors: { title: 'Temperature & fans', w: 4, h: 3, minW: 3, minH: 2 },
  network: { title: 'Network', w: 4, h: 3, minW: 2, minH: 2 },
  disk: { title: 'Disk I/O', w: 4, h: 3, minW: 2, minH: 2 },
  gpu: { title: 'GPU', w: 4, h: 3, minW: 2, minH: 2 },
  suggestions: { title: 'Suggestions', w: 4, h: 5, minW: 3, minH: 3 },
};

export interface WidgetPlacement {
  id: string;
  kind: WidgetKind;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface LayoutDoc {
  version: 1;
  widgets: WidgetPlacement[];
}

/** First-run layout: three small panels on the top row, suggestions beside them, processes below. */
export function defaultLayout(): LayoutDoc {
  const widgets: WidgetPlacement[] = [
    { id: 'cpu-1', kind: 'cpu', x: 0, y: 0, w: 4, h: 3 },
    { id: 'memory-1', kind: 'memory', x: 4, y: 0, w: 4, h: 3 },
    { id: 'sensors-1', kind: 'sensors', x: 8, y: 0, w: 4, h: 3 },
    { id: 'suggestions-1', kind: 'suggestions', x: 0, y: 3, w: 4, h: 5 },
    { id: 'processes-1', kind: 'processes', x: 4, y: 3, w: 8, h: 7 },
  ];
  return { version: 1, widgets };
}

/** Next free id for a kind, e.g. `cpu-2` if `cpu-1` already exists. */
export function nextWidgetId(kind: WidgetKind, existing: readonly string[]): string {
  const taken = new Set(existing);
  let n = 1;
  while (taken.has(`${kind}-${n}`)) n += 1;
  return `${kind}-${n}`;
}

export function serializeLayout(widgets: readonly WidgetPlacement[]): string {
  const doc: LayoutDoc = { version: 1, widgets: widgets.map((w) => ({ ...w })) };
  return JSON.stringify(doc, null, 2);
}

function isInt(v: unknown, min: number, max: number): v is number {
  return typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max;
}

/**
 * Parses and validates a saved layout. Returns `null` for anything malformed, so a
 * tampered or damaged file can never create an unknown widget or an off-grid placement.
 */
export function parseLayout(text: string): LayoutDoc | null {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return null;
  }
  if (typeof data !== 'object' || data === null) return null;
  const doc = data as { version?: unknown; widgets?: unknown };
  if (doc.version !== 1 || !Array.isArray(doc.widgets)) return null;
  if (doc.widgets.length > MAX_WIDGETS) return null;

  const seen = new Set<string>();
  const widgets: WidgetPlacement[] = [];
  for (const raw of doc.widgets) {
    if (typeof raw !== 'object' || raw === null) return null;
    const w = raw as Record<string, unknown>;
    const kind = w.kind;
    if (typeof kind !== 'string' || !(WIDGET_KINDS as readonly string[]).includes(kind)) return null;
    const id = w.id;
    if (typeof id !== 'string' || id.length === 0 || id.length > 64 || seen.has(id)) return null;
    seen.add(id);
    if (!isInt(w.x, 0, GRID_COLUMNS - 1) || !isInt(w.w, 1, GRID_COLUMNS)) return null;
    if (!isInt(w.y, 0, MAX_ROWS) || !isInt(w.h, 1, MAX_ROWS)) return null;
    const x = w.x as number;
    const width = w.w as number;
    if (x + width > GRID_COLUMNS) return null;
    widgets.push({
      id,
      kind: kind as WidgetKind,
      x,
      y: w.y as number,
      w: width,
      h: w.h as number,
    });
  }
  return { version: 1, widgets };
}
