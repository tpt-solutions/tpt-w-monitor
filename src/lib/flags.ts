// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// One-time UI flags (for example "onboarding dismissed"). Storage can be blocked, so
// every access is guarded and the app works without it.

const PREFIX = 'tpt-monitor.flag.';

export function getFlag(name: string): boolean {
  try {
    return globalThis.localStorage?.getItem(PREFIX + name) === '1';
  } catch {
    return false;
  }
}

export function setFlag(name: string): void {
  try {
    globalThis.localStorage?.setItem(PREFIX + name, '1');
  } catch {
    // Not persisted: the message simply shows again next launch.
  }
}
