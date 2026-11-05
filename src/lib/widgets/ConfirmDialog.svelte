<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  let {
    title,
    message,
    confirmLabel,
    onConfirm,
    onCancel,
  }: {
    title: string;
    message: string;
    confirmLabel: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') onCancel();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="overlay">
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-message">
    <h3 id="confirm-title">{title}</h3>
    <p id="confirm-message">{message}</p>
    <div class="dialog-actions">
      <button type="button" onclick={onCancel}>Cancel</button>
      <button type="button" class="danger" onclick={onConfirm}>{confirmLabel}</button>
    </div>
  </div>
</div>
