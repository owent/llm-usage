<script lang="ts">
  /**
   * 分组维度分段选择器（替代下拉）：总用量 | 按模型 | 按 Agent | 按 Agent+模型。
   * 水平按钮组（iOS segmented control 风格）：选中项蓝底白字，未选中白底灰边框；
   * 按钮间距 2px、圆角 6px、整体一行（父级 .dim-row 内与错误文案并排）。
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
