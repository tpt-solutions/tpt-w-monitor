<script lang="ts">
  // SPDX-License-Identifier: MIT OR Apache-2.0
  // Copyright (c) 2026 TPT Solutions

  import WidgetCard from './WidgetCard.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { live } from '../live.svelte';
  import { runAction } from '../actions';
  import { filterProcesses, sortProcesses, type SortKey } from '../processes';
  import { formatBytes, formatPercent, formatRate } from '../format';
  import type { PriorityDto, ProcessDto } from '../types';

  let { onRemove }: { onRemove: () => void } = $props();

  /** Rows rendered at once. The full list is still sorted and filtered. */
  const ROW_LIMIT = 300;

  let query = $state('');
  let sortKey = $state<SortKey>('cpu');
  let descending = $state(true);
  let pendingKill = $state<ProcessDto | null>(null);

  const all = $derived(live.frame?.processes ?? []);
  const rows = $derived(
    sortProcesses(filterProcesses(all, query), sortKey, descending).slice(0, ROW_LIMIT),
  );
  const matching = $derived(filterProcesses(all, query).length);

  function sortBy(key: SortKey) {
    if (sortKey === key) {
      descending = !descending;
    } else {
      sortKey = key;
      descending = key !== 'name' && key !== 'pid';
    }
  }

  function arrow(key: SortKey): string {
    if (sortKey !== key) return '';
    return descending ? ' ↓' : ' ↑';
  }

  function totalDisk(p: ProcessDto): number | null {
    if (p.diskReadBytesPerSec === null && p.diskWriteBytesPerSec === null) return null;
    return (p.diskReadBytesPerSec ?? 0) + (p.diskWriteBytesPerSec ?? 0);
  }

  async function suspend(p: ProcessDto) {
    await runAction(p.pid, { kind: 'suspend' });
  }

  async function resume(p: ProcessDto) {
    await runAction(p.pid, { kind: 'resume' });
  }

  async function setPriority(p: ProcessDto, priority: PriorityDto) {
    await runAction(p.pid, { kind: 'set_priority', priority });
  }

  async function confirmKill() {
    const target = pendingKill;
    pendingKill = null;
    if (target) await runAction(target.pid, { kind: 'kill' }, true);
  }
</script>

<WidgetCard title="Processes" {onRemove}>
  <div class="toolbar">
    <input type="search" placeholder="Filter by name or PID" aria-label="Filter processes" bind:value={query} />
    <span class="muted small">{matching} match{matching === 1 ? '' : 'es'}{matching > ROW_LIMIT ? `, showing first ${ROW_LIMIT}` : ''}</span>
  </div>

  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th><button type="button" onclick={() => sortBy('name')}>Name{arrow('name')}</button></th>
          <th><button type="button" onclick={() => sortBy('pid')}>PID{arrow('pid')}</button></th>
          <th class="num"><button type="button" onclick={() => sortBy('cpu')}>CPU{arrow('cpu')}</button></th>
          <th class="num"><button type="button" onclick={() => sortBy('memory')}>Memory{arrow('memory')}</button></th>
          <th class="num"><button type="button" onclick={() => sortBy('disk')}>Disk{arrow('disk')}</button></th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as p (p.pid)}
          <tr>
            <td class="name" title={p.name}>{p.name}</td>
            <td>{p.pid}</td>
            <td class="num">{formatPercent(p.cpuPercent)}</td>
            <td class="num">{formatBytes(p.workingSetBytes)}</td>
            <td class="num">{formatRate(totalDisk(p))}</td>
            <td class="actions">
              <button type="button" onclick={() => suspend(p)} aria-label="Suspend {p.name}">Suspend</button>
              <button type="button" onclick={() => resume(p)} aria-label="Resume {p.name}">Resume</button>
              <select aria-label="Priority for {p.name}" onchange={(e) => setPriority(p, e.currentTarget.value as PriorityDto)}>
                <option value="" selected disabled>Priority</option>
                <option value="idle">Idle</option>
                <option value="below_normal">Below normal</option>
                <option value="normal">Normal</option>
                <option value="above_normal">Above normal</option>
                <option value="high">High</option>
              </select>
              <button type="button" class="danger" onclick={() => (pendingKill = p)} aria-label="Kill {p.name}">Kill</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</WidgetCard>

{#if pendingKill}
  <ConfirmDialog
    title="Kill {pendingKill.name}?"
    message="Killing a process ends it immediately. Unsaved work in {pendingKill.name} will be lost. This cannot be undone."
    confirmLabel="Kill process"
    onConfirm={confirmKill}
    onCancel={() => (pendingKill = null)}
  />
{/if}
