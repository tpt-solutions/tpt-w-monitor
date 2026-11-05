<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  // Suggest-only: nothing runs until the user clicks Apply. Kill is never offered
  // here, and the backend re-checks every action.

  import WidgetCard from './WidgetCard.svelte';
  import { live, notify } from '../live.svelte';
  import { ipc } from '../ipc';
  import { runAction } from '../actions';
  import { formatPercent } from '../format';

  let { onRemove }: { onRemove: () => void } = $props();

  const suggestions = $derived(live.frame?.suggestions ?? []);

  async function apply(pid: number, action: (typeof suggestions)[number]['recommendedAction']) {
    await runAction(pid, action, false);
  }

  async function dismiss(name: string) {
    try {
      await ipc.dismissSuggestion(name);
    } catch (error) {
      notify(`Could not dismiss: ${String(error)}`);
    }
  }
</script>

<WidgetCard title="Suggestions" {onRemove}>
  {#if suggestions.length === 0}
    <p class="muted">Nothing to suggest right now. Suggestions appear after a background process stays busy for about 30 seconds.</p>
  {:else}
    <ul class="suggestions">
      {#each suggestions as s (s.pid)}
        <li>
          <p class="rationale">{s.rationale}</p>
          <p class="muted small">Confidence {formatPercent(s.confidence * 100)}</p>
          <div class="row-actions">
            <button type="button" onclick={() => apply(s.pid, s.recommendedAction)}>Apply</button>
            <button type="button" onclick={() => dismiss(s.processName)}>Don't suggest again</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</WidgetCard>
