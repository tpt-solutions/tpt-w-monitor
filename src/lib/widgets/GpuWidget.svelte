<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import { live } from '../live.svelte';
  import { formatBytes, formatPercent } from '../format';

  let { onRemove }: { onRemove: () => void } = $props();

  const gpuProcesses = $derived(
    (live.frame?.processes ?? [])
      .filter((p) => p.gpuEnginePercent !== null)
      .sort((a, b) => (b.gpuEnginePercent ?? 0) - (a.gpuEnginePercent ?? 0))
      .slice(0, 5),
  );
</script>

<WidgetCard title="GPU" {onRemove}>
  {#if gpuProcesses.length === 0}
    <p class="muted">GPU counters are not reported yet (Windows PDH and Linux DRM are in progress).</p>
  {:else}
    <ul class="readings">
      {#each gpuProcesses as p (p.pid)}
        <li>
          <span>{p.name} <small>({p.pid})</small></span>
          <strong>{formatPercent(p.gpuEnginePercent)} · {formatBytes(p.vramBytes)} VRAM</strong>
        </li>
      {/each}
    </ul>
  {/if}
</WidgetCard>
