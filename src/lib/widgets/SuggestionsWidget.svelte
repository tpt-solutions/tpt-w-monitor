<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  // Suggest-only: nothing runs until the user clicks Apply. Kill is never offered
  // here, and the backend re-checks every action.

  import { onMount } from 'svelte';
  import WidgetCard from './WidgetCard.svelte';
  import type { WidgetProps } from './props';
  import { live, notify } from '../live.svelte';
  import { ipc } from '../ipc';
  import { runAction } from '../actions';
  import { formatPercent } from '../format';
  import type { SuggestionSettingsDto } from '../types';

  let { onRemove, onPopOut }: WidgetProps = $props();

  const suggestions = $derived(live.frame?.suggestions ?? []);
  let settings = $state<SuggestionSettingsDto | null>(null);

  async function refresh() {
    try {
      settings = await ipc.getSuggestionSettings();
    } catch (error) {
      notify(`Could not load suggestion settings: ${String(error)}`);
    }
  }

  async function apply(pid: number, name: string, action: (typeof suggestions)[number]['recommendedAction']) {
    await runAction(pid, name, action, false);
  }

  async function dismiss(name: string) {
    try {
      await ipc.dismissSuggestion(name);
      await refresh();
    } catch (error) {
      notify(`Could not dismiss: ${String(error)}`);
    }
  }

  let newProtected = $state('');

  async function protect() {
    const name = newProtected.trim();
    if (name === '') return;
    try {
      await ipc.addProtected(name);
      newProtected = '';
      await refresh();
    } catch (error) {
      notify(`Could not protect: ${String(error)}`);
    }
  }

  async function unprotect(name: string) {
    try {
      await ipc.removeProtected(name);
      await refresh();
    } catch (error) {
      notify(`Could not remove: ${String(error)}`);
    }
  }

  async function restore(name: string) {
    try {
      await ipc.restoreSuggestion(name);
      await refresh();
    } catch (error) {
      notify(`Could not restore: ${String(error)}`);
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<WidgetCard title="Suggestions" {onRemove} {onPopOut}>
  {#if suggestions.length === 0}
    <p class="muted">Nothing to suggest right now. Suggestions appear after a background process stays busy for about 30 seconds.</p>
  {:else}
    <ul class="suggestions">
      {#each suggestions as s (s.pid)}
        <li>
          <p class="rationale">{s.rationale}</p>
          <p class="muted small">Confidence {formatPercent(s.confidence * 100)}</p>
          <div class="row-actions">
            <button type="button" onclick={() => apply(s.pid, s.processName, s.recommendedAction)}>Apply</button>
            <button type="button" onclick={() => dismiss(s.processName)}>Don't suggest again</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <details class="settings">
    <summary>Dismissed ({settings?.dismissed.length ?? 0})</summary>
    {#if settings && settings.dismissed.length > 0}
      <ul class="readings">
        {#each settings.dismissed as name (name)}
          <li>
            <span>{name}</span>
            <button type="button" onclick={() => restore(name)} aria-label="Suggest {name} again">Restore</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted small">No suggestions are dismissed.</p>
    {/if}
  </details>

  <details class="settings">
    <summary>Protected processes ({settings?.protected.length ?? 0})</summary>
    <p class="muted small">Never suggested for suspension or priority changes, and refused for kill or suspend even when you ask. You can add names; only names you added can be removed.</p>
    {#if settings}
      <ul class="protected">
        {#each settings.protected as name (name)}
          <li>
            <span>{name}</span>
            {#if settings.userProtected.includes(name)}
              <button type="button" onclick={() => unprotect(name)} aria-label="Remove protection for {name}">Remove</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
    <form class="row-actions" onsubmit={(e) => { e.preventDefault(); void protect(); }}>
      <input type="text" placeholder="executable.exe" aria-label="Process name to protect" bind:value={newProtected} maxlength="64" />
      <button type="submit">Protect</button>
    </form>
  </details>
</WidgetCard>

<style>
  .settings {
    margin-top: 10px;
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }

  summary {
    cursor: pointer;
    color: var(--muted);
  }

  .protected {
    columns: 2;
    margin: 4px 0 0;
    padding-left: 18px;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
  }
</style>
