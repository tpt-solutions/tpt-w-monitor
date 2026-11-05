<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  // One widget in its own borderless, always-on-top window. The widget itself is the
  // same component the dashboard uses; this page adds a toolbar for dragging, opacity,
  // click-through, docking back to the dashboard, and closing.

  import { onMount, mount, unmount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { ipc, onFrame, requestDock } from './lib/ipc';
  import { ingest, notify } from './lib/live.svelte';
  import { COMPONENTS } from './lib/registry';
  import { WIDGET_INFO, type WidgetKind } from './lib/layout';

  let { kind, id }: { kind: WidgetKind; id: string } = $props();

  let host: HTMLElement;
  let opacity = $state(1);
  let clickThrough = $state(false);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  const title = $derived(WIDGET_INFO[kind].title);

  /** Stores the window's current geometry and opacity, in logical pixels. */
  async function save() {
    try {
      const win = getCurrentWindow();
      const scale = await win.scaleFactor();
      const position = await win.outerPosition();
      const size = await win.innerSize();
      await ipc.updateOverlay(id, {
        x: Math.round(position.x / scale),
        y: Math.round(position.y / scale),
        width: Math.round(size.width / scale),
        height: Math.round(size.height / scale),
        opacity,
      });
    } catch (error) {
      console.warn('could not save overlay position', error);
    }
  }

  function scheduleSave() {
    if (saveTimer !== undefined) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void save(), 400);
  }

  async function dock() {
    try {
      await requestDock({ kind, id });
      await ipc.closeOverlay(id);
    } catch (error) {
      notify(`Could not dock the widget: ${String(error)}`);
    }
  }

  async function close() {
    try {
      await ipc.closeOverlay(id);
    } catch (error) {
      notify(`Could not close the overlay: ${String(error)}`);
    }
  }

  async function passClicksThrough() {
    try {
      await ipc.setClickThrough(id, true);
      clickThrough = true;
    } catch (error) {
      notify(`Could not enable click-through: ${String(error)}`);
    }
  }

  onMount(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else unlisteners.push(stop);
    };

    const widget = mount(COMPONENTS[kind], {
      target: host,
      props: { onRemove: () => void close() },
    });

    void ipc
      .listOverlays()
      .then((list) => {
        const mine = list.find((o) => o.id === id);
        if (mine) {
          opacity = mine.opacity;
          clickThrough = mine.clickThrough;
        }
      })
      .catch(() => undefined);

    void onFrame((frame) => ingest(frame)).then(keep);

    const win = getCurrentWindow();
    void win.onMoved(() => scheduleSave()).then(keep);
    void win.onResized(() => scheduleSave()).then(keep);

    return () => {
      disposed = true;
      unlisteners.forEach((stop) => stop());
      if (saveTimer !== undefined) clearTimeout(saveTimer);
      void unmount(widget);
    };
  });
</script>

<div class="overlay-root">
  <div class="overlay-bar" data-tauri-drag-region>
    <span class="overlay-title" data-tauri-drag-region>{title}</span>
    <input
      type="range"
      min="20"
      max="100"
      aria-label="Opacity"
      title="Opacity"
      value={Math.round(opacity * 100)}
      oninput={(event) => {
        opacity = Number(event.currentTarget.value) / 100;
        scheduleSave();
      }}
    />
    <button type="button" onclick={passClicksThrough} disabled={clickThrough} title="Let mouse clicks pass through to the desktop">Click-through</button>
    <button type="button" onclick={dock} title="Return this widget to the dashboard">Dock</button>
    <button type="button" class="icon" onclick={close} aria-label="Close overlay">×</button>
  </div>

  {#if clickThrough}
    <p class="overlay-note small">Click-through is on. Turn it off from the tray menu.</p>
  {/if}

  <div class="overlay-body" style="opacity: {opacity}" bind:this={host}></div>
</div>
