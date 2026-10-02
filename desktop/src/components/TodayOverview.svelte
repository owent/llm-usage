<script lang="ts">
  import type { MetricSumsDto, CostSummaryDto } from '../lib/api';
  import CostReferenceSummary from './CostReferenceSummary.svelte';
  import { t, fmtSmart, fmtPercent, fmtDurationShort } from '../lib/i18n.svelte';

  let {
    totals,
    sessions,
    costs=null,
    pricing=false,
    costsError='',
  }: {
    totals: MetricSumsDto;
    sessions: number | null;
    costs?: CostSummaryDto|null;
    pricing?: boolean;
    costsError?: string;
  } = $props();


  const cards = $derived.by(() => {
    const avgMs = totals.avg_duration_ms === null ? null : Number(totals.avg_duration_ms);
    const list: { key: string; value: string; hint?: string; sub?: string }[] = [
      { key: 'overview.today.calls', value: fmtSmart(totals.call_count) },
      {
        key: 'overview.today.input',
        value: fmtSmart(totals.input_total_known),
        hint: t('cards.input.hint'),
        sub: t('overview.breakdown.input', {
          hit: fmtSmart(totals.cache_read_known),
          miss: fmtSmart(totals.uncached_known),
        }),
      },
      { key: 'overview.today.output', value: fmtSmart(totals.output_total_known) },
      { key: 'overview.today.total', value: fmtSmart(totals.total_tokens_known) },
      { key: 'overview.today.cacheRatio', value: fmtPercent(totals.cache_input_ratio) },
      { key: 'overview.today.sessions', value: fmtSmart(sessions) },
      { key: 'overview.today.avgDuration', value: fmtDurationShort(avgMs) },
    ];
    return list;
  });
</script>

<div class="today-cards">
  {#each cards as c (c.key)}
    <div class="card" title={[c.hint,c.sub].filter(Boolean).join(' · ')}>
      <div class="label">
        {t(c.key)}
      </div>
      <div class="value">{c.value}</div>
    </div>
  {/each}
</div>
{#if pricing}<CostReferenceSummary summary={costs} error={costsError} />{/if}

<style>
  .today-cards {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 8px;
    padding: 8px 0 2px;
  }
  .card:first-child { background: var(--accent-bg); border-color: transparent; }
  .card:first-child .value { color: var(--accent); }
  .card {
    background: var(--bg-card-hover);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 12px 10px;
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: normal;
  }
  .value {
    font-size: 22px;
    font-weight: 650;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  @media(max-width:1200px) {.today-cards{grid-template-columns:repeat(4,minmax(0,1fr));}.card:last-child{grid-column:span 2;}}
  @media(max-width:600px) {.today-cards{grid-template-columns:repeat(2,minmax(0,1fr));}}
</style>
