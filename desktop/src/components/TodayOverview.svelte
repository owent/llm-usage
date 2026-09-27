<script lang="ts">
  import type { MetricSumsDto } from '../lib/api';
  import { t, fmtSmart, fmtPercent, fmtDurationShort } from '../lib/i18n.svelte';

  let {
    totals,
    sessions,
  }: {
    totals: MetricSumsDto;
    sessions: number | null;
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
    <div class="card" title={c.hint ?? ''}>
      <div class="label">
        {t(c.key)}{#if c.hint}<span class="hint">{c.hint}</span>{/if}
      </div>
      <div class="value">{c.value}</div>
      {#if c.sub}<div class="sub">{c.sub}</div>{/if}
    </div>
  {/each}
</div>

<style>
  .today-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 12px;
    padding: 8px 0 2px;
  }
  .card:first-child { background: var(--accent-bg); border-color: transparent; }
  .card:first-child .value { color: var(--accent); }
  .card {
    background: var(--bg-card-hover);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px 14px;
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: normal;
  }
  .label .hint {
    color: var(--text-muted);
    font-size: 12px;
  }
  .value {
    font-size: 26px;
    font-weight: 650;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  /* 输入分解小字：命中 X · 未命中 Y。 */
  .sub {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
</style>
