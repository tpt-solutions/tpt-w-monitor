<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import type { Snippet } from 'svelte';

  let {
    title,
    onRemove,
    onPopOut,
    children,
  }: { title: string; onRemove: () => void; onPopOut?: () => void; children: Snippet } = $props();
</script>

<section class="card">
  <header class="card-head">
    <h2>{title}</h2>
    <div class="card-tools">
      {#if onPopOut}
        <button class="icon" type="button" aria-label="Pop out {title} widget" title="Pop out to a desktop window" onclick={onPopOut}>⧉</button>
      {/if}
      <button class="icon" type="button" aria-label="Remove {title} widget" title="Remove widget" onclick={onRemove}>×</button>
    </div>
  </header>
  <div class="card-body">
    {@render children()}
  </div>
</section>
