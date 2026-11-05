<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import type { WidgetProps } from './props';
  import Sparkline from './Sparkline.svelte';
  import { live } from '../live.svelte';
  import { formatBytes, formatPercent } from '../format';

  let { onRemove, onPopOut }: WidgetProps = $props();

  const used = $derived(live.frame?.system.memoryUsedBytes ?? null);
  const total = $derived(live.frame?.system.memoryTotalBytes ?? null);
  const percent = $derived(
    used !== null && total ? (used / total) * 100 : null,
  );
</script>

<WidgetCard title="Memory" {onRemove} {onPopOut}>
  <div class="reading">{formatPercent(percent)}</div>
  <p class="muted">{formatBytes(used)} of {formatBytes(total)} in use</p>
  <Sparkline values={live.memory} label="Memory history" />
</WidgetCard>
