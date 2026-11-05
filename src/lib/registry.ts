// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Maps each widget kind to its Svelte component. Shared by the dashboard grid and
// the desktop overlay page, so a widget looks and behaves the same in both.

import type { Component } from 'svelte';
import type { WidgetKind } from './layout';
import type { WidgetProps } from './widgets/props';
import CpuWidget from './widgets/CpuWidget.svelte';
import MemoryWidget from './widgets/MemoryWidget.svelte';
import SensorsWidget from './widgets/SensorsWidget.svelte';
import NetworkWidget from './widgets/NetworkWidget.svelte';
import DiskWidget from './widgets/DiskWidget.svelte';
import GpuWidget from './widgets/GpuWidget.svelte';
import SuggestionsWidget from './widgets/SuggestionsWidget.svelte';
import ProcessesWidget from './widgets/ProcessesWidget.svelte';

export const COMPONENTS: Record<WidgetKind, Component<WidgetProps>> = {
  cpu: CpuWidget,
  memory: MemoryWidget,
  processes: ProcessesWidget,
  sensors: SensorsWidget,
  network: NetworkWidget,
  disk: DiskWidget,
  gpu: GpuWidget,
  suggestions: SuggestionsWidget,
};
