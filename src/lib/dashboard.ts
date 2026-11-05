// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Bridges gridstack (drag, resize, rearrange) and Svelte widget components. Gridstack
// owns the cell geometry; each cell's content is a Svelte component mounted here.

import { GridStack, type GridItemHTMLElement, type GridStackWidget } from 'gridstack';
import { mount, unmount, type Component } from 'svelte';
import {
  GRID_COLUMNS,
  MAX_WIDGETS,
  WIDGET_INFO,
  nextWidgetId,
  type LayoutDoc,
  type WidgetKind,
  type WidgetPlacement,
} from './layout';
import CpuWidget from './widgets/CpuWidget.svelte';
import MemoryWidget from './widgets/MemoryWidget.svelte';
import SensorsWidget from './widgets/SensorsWidget.svelte';
import NetworkWidget from './widgets/NetworkWidget.svelte';
import DiskWidget from './widgets/DiskWidget.svelte';
import GpuWidget from './widgets/GpuWidget.svelte';
import SuggestionsWidget from './widgets/SuggestionsWidget.svelte';
import ProcessesWidget from './widgets/ProcessesWidget.svelte';

type WidgetProps = { onRemove: () => void };

const COMPONENTS: Record<WidgetKind, Component<WidgetProps>> = {
  cpu: CpuWidget,
  memory: MemoryWidget,
  processes: ProcessesWidget,
  sensors: SensorsWidget,
  network: NetworkWidget,
  disk: DiskWidget,
  gpu: GpuWidget,
  suggestions: SuggestionsWidget,
};

interface Mounted {
  kind: WidgetKind;
  el: GridItemHTMLElement;
  app: Record<string, unknown>;
}

export interface Dashboard {
  /** Adds a widget at the first free position. Returns false when the limit is reached. */
  add(kind: WidgetKind): boolean;
  /** Current placements, as saved in a layout file. */
  layout(): WidgetPlacement[];
  /** Replaces every widget with the ones in `doc`. */
  load(doc: LayoutDoc): void;
  /** Unmounts all widgets and removes the grid. */
  destroy(): void;
}

/**
 * Creates the grid inside `container`. `onChange` fires after the user moves,
 * resizes, adds or removes a widget, so the caller can autosave.
 */
export function createDashboard(container: HTMLElement, onChange: () => void): Dashboard {
  const started = GridStack.init(
    {
      column: GRID_COLUMNS,
      cellHeight: 96,
      margin: 8,
      animate: true,
      handle: '.card-head',
      columnOpts: { breakpoints: [{ w: 700, c: 1 }] },
    },
    container,
  );
  if (!started) throw new Error('gridstack could not start on the dashboard container');
  const grid: GridStack = started;

  const mounted = new Map<string, Mounted>();

  function unmountWidget(id: string, removeDom: boolean) {
    const item = mounted.get(id);
    if (!item) return;
    unmount(item.app);
    mounted.delete(id);
    if (removeDom) grid.removeWidget(item.el, true, false);
  }

  function mountWidget(id: string, kind: WidgetKind, pos: Partial<GridStackWidget>) {
    const info = WIDGET_INFO[kind];
    const el = grid.addWidget({
      ...pos,
      id,
      w: pos.w ?? info.w,
      h: pos.h ?? info.h,
      minW: info.minW,
      minH: info.minH,
    });
    if (!el) throw new Error(`gridstack refused to place widget ${id}`);
    const target = el.querySelector<HTMLElement>('.grid-stack-item-content');
    if (!target) throw new Error('gridstack did not create a content element');
    const app = mount(COMPONENTS[kind], {
      target,
      props: { onRemove: () => remove(id) },
    });
    mounted.set(id, { kind, el, app: app as Record<string, unknown> });
  }

  function remove(id: string) {
    unmountWidget(id, true);
    onChange();
  }

  grid.on('change', () => onChange());

  return {
    add(kind) {
      if (mounted.size >= MAX_WIDGETS) return false;
      const id = nextWidgetId(kind, [...mounted.keys()]);
      mountWidget(id, kind, {});
      onChange();
      return true;
    },

    layout() {
      const placements: WidgetPlacement[] = [];
      for (const node of grid.save(false) as GridStackWidget[]) {
        const id = String(node.id ?? '');
        const item = mounted.get(id);
        if (!item || node.x === undefined || node.y === undefined) continue;
        placements.push({
          id,
          kind: item.kind,
          x: node.x,
          y: node.y,
          w: node.w ?? WIDGET_INFO[item.kind].w,
          h: node.h ?? WIDGET_INFO[item.kind].h,
        });
      }
      return placements;
    },

    load(doc) {
      for (const id of [...mounted.keys()]) unmountWidget(id, false);
      grid.removeAll(true, false);
      for (const p of doc.widgets) {
        mountWidget(p.id, p.kind, { x: p.x, y: p.y, w: p.w, h: p.h });
      }
    },

    destroy() {
      for (const id of [...mounted.keys()]) unmountWidget(id, false);
      grid.destroy(false);
    },
  };
}
