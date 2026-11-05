<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import { live } from '../live.svelte';
  import { formatRate } from '../format';

  let { onRemove }: { onRemove: () => void } = $props();

  // Per-drive totals are not available yet, so this lists the busiest processes by disk I/O.
  const busiest = $derived(
    (live.frame?.processes ?? [])
      .filter((p) => p.diskReadBytesPerSec !== null || p.diskWriteBytesPerSec !== null)
      .map((p) => ({
        pid: p.pid,
        name: p.name,
        total: (p.diskReadBytesPerSec ?? 0) + (p.diskWriteBytesPerSec ?? 0),
      }))
      .sort((a, b) => b.total - a.total)
      .slice(0, 5),
  );
</script>

<WidgetCard title="Disk I/O" {onRemove}>
  {#if busiest.length === 0}
    <p class="muted">Disk I/O is not reported yet. Per-drive totals are still to come.</p>
  {:else}
    <p class="muted">Busiest processes (per-drive totals to come)</p>
    <ul class="readings">
      {#each busiest as p (p.pid)}
        <li><span>{p.name} <small>({p.pid})</small></span><strong>{formatRate(p.total)}</strong></li>
      {/each}
    </ul>
  {/if}
</WidgetCard>
