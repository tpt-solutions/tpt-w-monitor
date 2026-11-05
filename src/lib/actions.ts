// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { ipc } from './ipc';
import { notify } from './live.svelte';
import type { ActionDto } from './types';

/**
 * Sends an action to the backend and reports any refusal in the banner. The backend
 * is the authority: it re-checks PID, critical-process and confirmation rules.
 */
export async function runAction(pid: number, action: ActionDto, confirmed = false): Promise<boolean> {
  try {
    await ipc.applyAction(pid, action, confirmed);
    return true;
  } catch (error) {
    notify(`Action failed: ${String(error)}`);
    return false;
  }
}
