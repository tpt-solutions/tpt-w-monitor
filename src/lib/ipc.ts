// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.

import { invoke } from '@tauri-apps/api/core';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ActionDto,
  CapabilitiesDto,
  FrameDto,
  OverlayDto,
  SuggestionSettingsDto,
  ViewDto,
} from './types';
import type { WidgetKind } from './layout';

/** Sent by an overlay when the user docks it back into the dashboard. */
export const EVENT_OVERLAY_DOCK = 'overlay-dock';

export interface DockRequest {
  kind: WidgetKind;
  id: string;
}

export const ipc = {
  getFrame: () => invoke<FrameDto | null>('get_frame'),
  applyAction: (pid: number, action: ActionDto, confirmed: boolean) =>
    invoke<void>('apply_action', { pid, action, confirmed }),
  setViewState: (view: ViewDto) => invoke<void>('set_view_state', { view }),
  dismissSuggestion: (processName: string) => invoke<void>('dismiss_suggestion', { processName }),
  restoreSuggestion: (processName: string) => invoke<void>('restore_suggestion', { processName }),
  saveLayout: (name: string, content: string) => invoke<void>('save_layout', { name, content }),
  loadLayout: (name: string) => invoke<string | null>('load_layout', { name }),
  listLayouts: () => invoke<string[]>('list_layouts'),
  openOverlay: (kind: WidgetKind, id: string) => invoke<OverlayDto>('open_overlay', { kind, id }),
  closeOverlay: (id: string) => invoke<void>('close_overlay', { id }),
  updateOverlay: (
    id: string,
    geometry: { x: number; y: number; width: number; height: number; opacity: number },
  ) => invoke<void>('update_overlay', { id, ...geometry }),
  setClickThrough: (id: string, enabled: boolean) => invoke<void>('set_click_through', { id, enabled }),
  listOverlays: () => invoke<OverlayDto[]>('list_overlays'),
  getSuggestionSettings: () => invoke<SuggestionSettingsDto>('get_suggestion_settings'),
  getCapabilities: () => invoke<CapabilitiesDto>('get_capabilities'),
  addProtected: (name: string) => invoke<void>('add_protected', { name }),
  removeProtected: (name: string) => invoke<void>('remove_protected', { name }),
};

/** Subscribes to pushed frames. Resolves to the unsubscribe function. */
export function onFrame(handler: (frame: FrameDto) => void): Promise<UnlistenFn> {
  return listen<FrameDto>('metrics', (event) => handler(event.payload));
}

/** Tells the main window that an overlay's widget should return to the dashboard. */
export function requestDock(request: DockRequest): Promise<void> {
  return emit(EVENT_OVERLAY_DOCK, request);
}

/** Listens for dock requests from overlays. Resolves to the unsubscribe function. */
export function onDockRequest(handler: (request: DockRequest) => void): Promise<UnlistenFn> {
  return listen<DockRequest>(EVENT_OVERLAY_DOCK, (event) => handler(event.payload));
}
