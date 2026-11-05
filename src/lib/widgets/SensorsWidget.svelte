<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import { live } from '../live.svelte';
  import { formatCelsius, formatRpm } from '../format';

  let { onRemove }: { onRemove: () => void } = $props();

  const temps = $derived(live.frame?.sensors.temperatures ?? null);
  const fans = $derived(live.frame?.sensors.fans ?? null);
</script>

<WidgetCard title="Temperature & fans" {onRemove}>
  {#if temps === null}
    <p class="muted">Temperature sensors are unavailable on this system.</p>
  {:else if temps.length === 0}
    <p class="muted">Sensor access works, but no temperature sensors reported.</p>
  {:else}
    <ul class="readings">
      {#each temps as t (t.label)}
        <li><span>{t.label}</span><strong>{formatCelsius(t.celsius)}</strong></li>
      {/each}
    </ul>
  {/if}

  {#if fans === null}
    <p class="muted">Fan sensors are unavailable on this system.</p>
  {:else if fans.length > 0}
    <ul class="readings">
      {#each fans as f (f.label)}
        <li><span>{f.label}</span><strong>{formatRpm(f.rpm)}</strong></li>
      {/each}
    </ul>
  {/if}
</WidgetCard>
