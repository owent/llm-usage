<script lang="ts">
  /**
   * 今日汇总卡片（任务 D8）：调用/输入（含缓存命中）/输出/总量/缓存命中率/会话数/平均耗时。
   * 总量指标来自今日 summary 的 totals；会话数按 today_hourly 汇总（任务 A2：
   * 输入显示 input_total_known 总量而非 uncached）。数值用 fmtSmart 缩放（任务 B3）。
   * 输入 token 卡片（2026-09-26）：主数字 = 总输入，下方小字显示
   * 缓存命中/未命中分解（未命中用后端 uncached_known，未知不补零显示 —）。
   */
  import type { MetricSumsDto, SummaryDto } from '../lib/api';
  import { t, fmtSmart, fmtPercent, fmtDurationShort } from '../lib/i18n.svelte';

  let {
    totals,
    hourly,
  }: {
    totals: MetricSumsDto;
    hourly: SummaryDto['today_hourly'];
  } = $props();

  // 会话数：今日各小时 session_count 求和（totals 不含会话维度）。
  const sessions = $derived(hourly.reduce((s, h) => s + (h.sessions ?? 0), 0));

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
    gap: 8px;
    padding: 8px 0 2px;
  }
  .card {
    background: var(--bg-code);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 12px;
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .label .hint {
    color: var(--text-muted);
    font-size: 11px;
  }
  .value {
    font-size: 18px;
    font-weight: 600;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  /* 输入分解小字：命中 X · 未命中 Y。 */
  .sub {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
</style>
