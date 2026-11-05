// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

export type ThemeChoice = 'system' | 'light' | 'dark';

const KEY = 'tpt-monitor.theme';

/** Reads the saved theme. Storage can be blocked, so every access is guarded. */
export function loadTheme(): ThemeChoice {
  try {
    const value = globalThis.localStorage?.getItem(KEY);
    if (value === 'light' || value === 'dark' || value === 'system') return value;
  } catch {
    // Storage unavailable: fall through to the default.
  }
  return 'system';
}

export function saveTheme(choice: ThemeChoice): void {
  try {
    globalThis.localStorage?.setItem(KEY, choice);
  } catch {
    // Not persisted this time; the theme still applies for this session.
  }
}

/** Sets `data-theme` on the root element. `system` removes it so the CSS media query decides. */
export function applyTheme(choice: ThemeChoice, root: HTMLElement = document.documentElement): void {
  if (choice === 'system') {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = choice;
  }
}
