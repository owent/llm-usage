<script lang="ts">
  import type { MetricSumsDto } from '../lib/api';
  import { t, fmtNumber, fmtPercent } from '../lib/i18n.svelte';

  let { totals, excluded = 0 }: { totals: MetricSumsDto; excluded?: number } = $props();

  const cards = $derived([
    { key: 'cards.calls', value: fmtNumber(totals.call_count) },
    { key: 'cards.input', value: fmtNumber(totals.input_total_known) },
    { key: 'cards.output', value: fmtNumber(totals.output_total_known) },
    { key: 'cards.total', value: fmtNumber(totals.total_tokens_known) },
    { key: 'cards.cacheRead', value: fmtNumber(totals.cache_read_known) },
    { key: 'cards.cacheWrite', value: fmtNumber(totals.cache_write_known) },
    { key: 'cards.cacheRatio', value: fmtPercent(totals.cache_input_ratio) },
  ]);
</script>

<div class="cards">
  {#each cards as c (c.key)}
    <div class="card">
      <div class="label">{t(c.key)}</div>
      <div class="value">{c.value}</div>
    </div>
  {/each}
</div>
{#if totals.conflict_count > 0 || excluded > 0}
  <p class="note">
    {#if totals.conflict_count > 0}{t('cards.conflicts')}: {fmtNumber(totals.conflict_count)}{/if}
    {#if excluded > 0}{t('cards.excluded')}: {fmtNumber(excluded)}{/if}
  </p>
{/if}

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px;
    padding: 8px 0;
  }
  .card {
    background: #f7f8fa;
    border: 1px solid #e6e8eb;
    border-radius: 8px;
    padding: 10px 14px;
  }
  .label {
    font-size: 12px;
    color: #666;
  }
  .value {
    font-size: 20px;
    font-weight: 600;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  .note {
    font-size: 12px;
    color: #8a6d1a;
  }
</style>
