<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import { onMount } from 'svelte';
  import { createDashboard, type Dashboard } from './lib/dashboard';
  import { ipc, onDockRequest, onFrame } from './lib/ipc';
  import { ingest, live, notify } from './lib/live.svelte';
  import {
    WIDGET_INFO,
    WIDGET_KINDS,
    defaultLayout,
    parseLayout,
    serializeLayout,
    type WidgetKind,
  } from './lib/layout';
  import { applyTheme, loadTheme, saveTheme, type ThemeChoice } from './lib/theme';
  import Onboarding from './lib/widgets/Onboarding.svelte';
  import type { CapabilitiesDto } from './lib/types';

  /** Layout restored on start and saved after every change. */
  const AUTOSAVE = 'last-session';

  let gridHost: HTMLElement;
  let dashboard: Dashboard | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  let theme = $state<ThemeChoice>(loadTheme());
  let palette = $state<WidgetKind>('cpu');
  let layoutName = $state('my-layout');
  let savedLayouts = $state<string[]>([]);
  let selectedLayout = $state('');
  let capabilities = $state<CapabilitiesDto | null>(null);
  let lastUpdate = $state('');

  $effect(() => {
    applyTheme(theme);
    saveTheme(theme);
  });

  function scheduleAutosave() {
    if (saveTimer !== undefined) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void persist(AUTOSAVE), 500);
  }

  /** Writes the current layout under `name`. Returns whether the write succeeded. */
  async function persist(name: string): Promise<boolean> {
    if (!dashboard) return false;
    try {
      await ipc.saveLayout(name, serializeLayout(dashboard.layout()));
      return true;
    } catch (error) {
      notify(`Could not save layout: ${String(error)}`);
      return false;
    }
  }

  async function refreshLayoutList() {
    try {
      savedLayouts = await ipc.listLayouts();
    } catch {
      savedLayouts = [];
    }
  }

  /** A widget left the grid for its own window. If opening fails, it returns to the grid. */
  async function popOutWidget(kind: WidgetKind, id: string) {
    try {
      await ipc.openOverlay(kind, id);
      notify(`${WIDGET_INFO[kind].title} is now in its own window. Use its dock button to bring it back.`);
    } catch (error) {
      dashboard?.add(kind);
      notify(`Could not open the overlay: ${String(error)}`);
    }
  }

  function addWidget() {
    if (!dashboard?.add(palette)) notify('Widget limit reached. Remove one to add another.');
  }

  function resetLayout() {
    dashboard?.load(defaultLayout());
    scheduleAutosave();
  }

  async function saveNamed() {
    const name = layoutName.trim();
    if (!(await persist(name))) return;
    selectedLayout = name;
    await refreshLayoutList();
    notify(`Layout saved as “${name}”.`);
  }

  async function loadNamed() {
    if (!selectedLayout) return;
    let raw: string | null;
    try {
      raw = await ipc.loadLayout(selectedLayout);
    } catch (error) {
      notify(`Could not read layout: ${String(error)}`);
      return;
    }
    const doc = raw === null ? null : parseLayout(raw);
    if (!doc) {
      notify('That layout is missing or damaged, so it was not loaded.');
      return;
    }
    dashboard?.load(doc);
    scheduleAutosave();
    notify(`Loaded “${selectedLayout}”.`);
  }

  onMount(() => {
    let disposed = false;
    void ipc
      .getCapabilities()
      .then((c) => {
        capabilities = c;
      })
      .catch(() => undefined);
    let unlisten: (() => void) | undefined;
    let unlistenDock: (() => void) | undefined;

    // An overlay's dock button sends its widget back to the grid.
    void onDockRequest(({ kind }) => {
      // The payload comes from another window, so it is checked before use.
      if (!(WIDGET_KINDS as readonly string[]).includes(kind)) return;
      if (dashboard?.add(kind)) notify(`${WIDGET_INFO[kind].title} is back on the dashboard.`);
      else notify('Widget limit reached. Remove one before docking another.');
    }).then((stop) => {
      if (disposed) stop();
      else unlistenDock = stop;
    });
    dashboard = createDashboard(gridHost, scheduleAutosave, (kind, id) => void popOutWidget(kind, id));

    void (async () => {
      let doc = null;
      try {
        const raw = await ipc.loadLayout(AUTOSAVE);
        doc = raw === null ? null : parseLayout(raw);
      } catch {
        doc = null;
      }
      dashboard?.load(doc ?? defaultLayout());
      await refreshLayoutList();

      const stop = await onFrame((frame) => {
        ingest(frame);
        lastUpdate = new Date().toLocaleTimeString();
      });
      if (disposed) stop();
      else unlisten = stop;

      try {
        const frame = await ipc.getFrame();
        if (frame) ingest(frame);
      } catch {
        // The next pushed frame will populate the dashboard.
      }
    })();

    // Tell the backend when the dashboard is visible and focused, so it can slow sampling.
    const reportView = () => {
      void ipc
        .listOverlays()
        .catch(() => [])
        .then((overlays) =>
          ipc.setViewState({
            dashboardVisible: document.visibilityState === 'visible',
            dashboardFocused: document.hasFocus(),
            openOverlays: overlays.length,
          }),
        )
        .catch(() => undefined);
    };
    reportView();
    document.addEventListener('visibilitychange', reportView);
    window.addEventListener('focus', reportView);
    window.addEventListener('blur', reportView);

    return () => {
      disposed = true;
      unlisten?.();
      unlistenDock?.();
      document.removeEventListener('visibilitychange', reportView);
      window.removeEventListener('focus', reportView);
      window.removeEventListener('blur', reportView);
      if (saveTimer !== undefined) clearTimeout(saveTimer);
      dashboard?.destroy();
      dashboard = undefined;
    };
  });
</script>

<div class="app">
  <header class="topbar">
    <h1>tpt-monitor</h1>
    <p class="muted small" aria-live="off">{lastUpdate ? `Updated ${lastUpdate}` : 'Waiting for first sample…'}</p>
    <div class="spacer"></div>

    <label>
      Theme
      <select bind:value={theme}>
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>
    <label>
      Widget
      <select bind:value={palette}>
        {#each WIDGET_KINDS as kind (kind)}
          <option value={kind}>{WIDGET_INFO[kind].title}</option>
        {/each}
      </select>
    </label>
    <button type="button" onclick={addWidget}>Add widget</button>
    <button type="button" onclick={resetLayout}>Reset layout</button>
  </header>

  <section class="layouts" aria-label="Saved layouts">
    <label>
      Layout name
      <input type="text" bind:value={layoutName} maxlength="64" />
    </label>
    <button type="button" onclick={saveNamed}>Save layout</button>
    <label>
      Saved layouts
      <select bind:value={selectedLayout}>
        <option value="">Choose…</option>
        {#each savedLayouts as name (name)}
          <option value={name}>{name}</option>
        {/each}
      </select>
    </label>
    <button type="button" onclick={loadNamed} disabled={!selectedLayout}>Load</button>
  </section>

  {#if capabilities}
    <Onboarding {capabilities} />
  {/if}

  {#if live.notice}
    <div class="banner" role="status">{live.notice}</div>
  {/if}

  <main class="grid-stack" bind:this={gridHost}></main>
</div>
