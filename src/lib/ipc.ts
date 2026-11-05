// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ActionDto, FrameDto, ViewDto } from './types';

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
};

/** Subscribes to pushed frames. Resolves to the unsubscribe function. */
export function onFrame(handler: (frame: FrameDto) => void): Promise<UnlistenFn> {
  return listen<FrameDto>('metrics', (event) => handler(event.payload));
}
