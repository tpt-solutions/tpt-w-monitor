<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import type { WidgetProps } from './props';
  import { live } from '../live.svelte';
  import { formatRate } from '../format';

  let { onRemove, onPopOut }: WidgetProps = $props();

  const up = $derived(live.frame?.system.netUpBytesPerSec ?? null);
  const down = $derived(live.frame?.system.netDownBytesPerSec ?? null);
</script>

<WidgetCard title="Network" {onRemove} {onPopOut}>
  {#if up === null && down === null}
    <p class="muted">Network metrics are not available yet on this platform (todo.md Phase 2 and 3).</p>
  {:else}
    <ul class="readings">
      <li><span>Upload</span><strong>{formatRate(up)}</strong></li>
      <li><span>Download</span><strong>{formatRate(down)}</strong></li>
    </ul>
  {/if}
</WidgetCard>
