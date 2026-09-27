<script lang="ts">
  import type { SummaryDto } from '../lib/api';
  import { t, fmtSmart, fmtPercent } from '../lib/i18n.svelte';
  let { summary }: { summary: SummaryDto } = $props();
  const samples = $derived(summary.totals.total_known_count + summary.totals.total_unknown_count);
  const knownRatio = $derived(samples && samples === summary.totals.event_count - summary.totals.attempt_count ? summary.totals.total_known_count / samples : null);
</script>

<div class="insights" aria-label={t('insights.title')}>
  <div><span>{t('insights.activeDays')}</span><strong>{fmtSmart(summary.active_days)}</strong></div>
  <div><span>{t('insights.knownUsage')}</span><strong>{fmtPercent(knownRatio)}</strong><small>{t('insights.knownHint')}</small></div>
  <div><span>{t('insights.durationSamples')}</span><strong>{fmtSmart(summary.totals.duration_sample_count)}</strong></div>
  <div class:attention={summary.totals.conflict_count > 0}><span>{t('insights.conflicts')}</span><strong>{fmtSmart(summary.totals.conflict_count)}</strong></div>
  <p>{summary.periods.some((p) => p.partial_history) ? t('insights.retained') : t('insights.note')}</p>
</div>

<style>
  .insights { display: flex; align-items: center; flex-wrap: wrap; gap: 20px 32px; padding: 18px 22px; margin: 16px 0; border: 1px solid var(--border); border-radius: 14px; background: var(--bg-card); }
  .insights > div { display: grid; grid-template-columns: auto auto; align-items: baseline; gap: 3px 12px; }
  span, small, p { color: var(--text-secondary); font-size: 12px; }
  strong { font-size: 20px; font-weight: 650; font-variant-numeric: tabular-nums; }
  small { grid-column: 1 / -1; color: var(--text-muted); }
  p { margin: 0; flex: 1; min-width: 190px; line-height: 1.7; }
  .attention strong { color: var(--warning); }
</style>
