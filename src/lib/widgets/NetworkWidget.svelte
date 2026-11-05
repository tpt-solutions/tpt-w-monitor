<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import type { WidgetProps } from './props';
  import { live } from '../live.svelte';
  import { formatRate } from '../format';

  let { onRemove, onPopOut }: WidgetProps = $props();

  // Whole-machine totals are not provided yet, so this lists the busiest TCP processes.
  // Only processes with traffic appear; UDP is not attributed on Windows yet.
  const busiest = $derived(
    (live.frame?.processes ?? [])
      .filter((p) => p.netUpBytesPerSec !== null || p.netDownBytesPerSec !== null)
      .map((p) => ({
        pid: p.pid,
        name: p.name,
        up: p.netUpBytesPerSec ?? 0,
        down: p.netDownBytesPerSec ?? 0,
      }))
      .filter((p) => p.up + p.down > 0)
      .sort((a, b) => b.up + b.down - (a.up + a.down))
      .slice(0, 5),
  );
</script>

<WidgetCard title="Network" {onRemove} {onPopOut}>
  {#if busiest.length === 0}
    <p class="muted">No TCP traffic reported. Per-process network needs an elevated run on Windows, and UDP is not attributed yet.</p>
  {:else}
    <p class="muted">Busiest processes (TCP; totals to come)</p>
    <ul class="readings">
      {#each busiest as p (p.pid)}
        <li>
          <span>{p.name} <small>({p.pid})</small></span>
          <strong>↑ {formatRate(p.up)} · ↓ {formatRate(p.down)}</strong>
        </li>
      {/each}
    </ul>
  {/if}
</WidgetCard>
