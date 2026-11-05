<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import type { Snippet } from 'svelte';

  let {
    title,
    onRemove,
    children,
  }: { title: string; onRemove: () => void; children: Snippet } = $props();
</script>

<section class="card">
  <header class="card-head">
    <h2>{title}</h2>
    <button class="icon" type="button" aria-label="Remove {title} widget" title="Remove widget" onclick={onRemove}>×</button>
  </header>
  <div class="card-body">
    {@render children()}
  </div>
</section>
