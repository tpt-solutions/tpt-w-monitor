<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  // First-run card. It explains what this particular run can see, based on the backend's
  // own report, not a generic guess. Dismissing it is remembered on this machine.

  import { getFlag, setFlag } from '../flags';
  import type { CapabilitiesDto } from '../types';

  let { capabilities }: { capabilities: CapabilitiesDto } = $props();

  let dismissed = $state(getFlag('onboarded'));

  function dismiss() {
    setFlag('onboarded');
    dismissed = true;
  }

  const elevationNote = $derived(
    capabilities.elevated
      ? 'This run has administrator rights, so every process, its network traffic and its GPU use are visible, and you can change them.'
      : capabilities.platform === 'windows'
        ? 'This run does not have administrator rights. Some processes are hidden, per-connection network counters are off, and some actions are refused. Start tpt-monitor from an elevated session to see everything.'
        : 'This run is not root. Some processes are hidden and some actions are refused. Start with sudo for full visibility.',
  );
</script>

{#if !dismissed}
  <section class="onboarding" aria-labelledby="onboarding-title">
    <h2 id="onboarding-title">Welcome to tpt-monitor</h2>
    <p>{elevationNote}</p>
    <p class="muted small">
      Temperature and fan sensors depend on your hardware. Where a sensor cannot be read safely, its widget says so
      rather than showing a guess. Suggestions never act on their own: you choose whether to apply them.
    </p>
    <div class="row-actions">
      <button type="button" onclick={dismiss}>Got it</button>
    </div>
  </section>
{/if}
