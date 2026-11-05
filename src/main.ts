// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { mount } from 'svelte';
import 'gridstack/dist/gridstack.min.css';
import './app.css';
import App from './App.svelte';
import OverlayApp from './OverlayApp.svelte';
import { WIDGET_KINDS, type WidgetKind } from './lib/layout';

const target = document.getElementById('app');
if (!target) throw new Error('missing #app mount point');

// The same page serves the dashboard and each desktop overlay. Overlay windows are
// opened with ?overlay=<kind>&id=<id> (see src-tauri/src/lib.rs).
const params = new URLSearchParams(window.location.search);
const overlayKind = params.get('overlay');
if (overlayKind !== null) {
  document.documentElement.classList.add('overlay-window');
  const id = params.get('id');
  const kind = (WIDGET_KINDS as readonly string[]).includes(overlayKind)
    ? (overlayKind as WidgetKind)
    : null;
  if (kind !== null && id !== null) {
    mount(OverlayApp, { target, props: { kind, id } });
  } else {
    target.textContent = 'This overlay refers to an unknown widget.';
  }
} else {
  mount(App, { target });
}
