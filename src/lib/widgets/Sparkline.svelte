<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import { toPolylinePoints } from '../history';

  let {
    values,
    max = 100,
    label = 'History graph',
  }: { values: number[]; max?: number; label?: string } = $props();

  const HEIGHT = 48;
  const points = $derived(toPolylinePoints(values, 100, HEIGHT, max));
</script>

<svg class="spark" viewBox="0 0 100 {HEIGHT}" preserveAspectRatio="none" role="img" aria-label={label}>
  <polyline {points} fill="none" stroke="var(--accent)" stroke-width="2" vector-effect="non-scaling-stroke" />
</svg>
