<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import Sparkline from './Sparkline.svelte';
  import { live } from '../live.svelte';
  import { formatPercent } from '../format';

  let { onRemove }: { onRemove: () => void } = $props();

  let perCore = $state(false);
  const total = $derived(live.frame?.system.cpuTotalPercent ?? null);
  const cores = $derived(live.frame?.system.cpuPerCorePercent ?? []);
</script>

<WidgetCard title="CPU" {onRemove}>
  <div class="reading">{formatPercent(total)}</div>
  <Sparkline values={live.cpu} label="CPU history" />
  <label class="toggle">
    <input type="checkbox" bind:checked={perCore} /> Per-core
  </label>
  {#if perCore}
    <div class="cores">
      {#each cores as load, i (i)}
        <div class="core" title="Core {i}: {Math.round(load)}%">
          <div class="bar"><span style="height: {Math.min(100, load)}%"></span></div>
          <small>{i}</small>
        </div>
      {/each}
    </div>
  {/if}
</WidgetCard>
