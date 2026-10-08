<script lang="ts">
  /**
   * Segmented grouping selector: total usage, model, Agent, or Agent plus model.
   * Selected buttons use blue with white text; others use white with gray borders.
   * Spacing is 2px, radius 6px, one row beside errors in the parent .dim-row.
   */
  import type { ChartDimension } from '../lib/api';
  import { t } from '../lib/i18n.svelte';

  let {
    value,
    onselect,
  }: {
    value: ChartDimension;
    onselect: (next: ChartDimension) => void;
  } = $props();

  const options = $derived.by(
    () =>
      [
        ['total', t('chart.dimension.total')],
        ['model', t('chart.dimension.model')],
        ['agent', t('chart.dimension.agent')],
        ['agent_model', t('chart.dimension.agentModel')],
      ] as [ChartDimension, string][]
  );
</script>

<div class="dim-seg">
  {#each options as [id, label] (id)}
    <button type="button" aria-pressed={value === id} class:active={value === id} onclick={() => onselect(id)}>
      {label}
    </button>
  {/each}
</div>

<style>
  .dim-seg {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    background: var(--bg-hover);
    border-radius: 8px;
    max-width: 100%;
    overflow-x: auto;
  }
  .dim-seg button {
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-secondary);
    border-radius: 6px;
    padding: 3px 10px;
    font-size: 12.5px;
    line-height: 1.4;
    cursor: pointer;
    white-space: nowrap;
  }
  .dim-seg button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .dim-seg button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
    font-weight: 600;
  }
</style>
