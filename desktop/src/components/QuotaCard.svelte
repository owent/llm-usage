<script lang="ts">
  /**
   * 通用额度卡片：展示某 Agent 的账户级/速率限额额度（请求或额度计数，非 token）
   * 与每日消耗趋势。数据来自通用 quota_history（agent 无关），任何未来 Agent 复用。
   * 首个接入者为 Copilot premium 请求额度。
   */
  import { api, parseError } from '../lib/api';
  import type { QuotaDto, QuotaPointDto } from '../lib/api';
  import { t, fmtSmart, i18n } from '../lib/i18n.svelte';

  let {
    agent,
    quotaId = 'premium_interactions',
    reloadKey = 0,
    isDark = false,
    timezone = 'UTC',
  }: {
    agent: string;
    quotaId?: string;
    reloadKey?: number;
    isDark?: boolean;
    timezone?: string;
  } = $props();

  const AGENT_LABELS: Record<string, string> = { copilot: 'GitHub Copilot' };
  const agentLabel = $derived(AGENT_LABELS[agent] ?? agent);

  let quotas = $state<QuotaDto[]>([]);
  let points = $state<QuotaPointDto[]>([]);
  let error = $state('');

  $effect(() => {
    const a = agent;
    const q = quotaId;
    const key = reloadKey;
    const tz = timezone;
    void key;
    void tz;
    error = '';
    quotas = [];
    points = [];
    let active = true;
    Promise.all([api.quotaSummary(a), api.quotaSeries(a, q)])
      .then(([summary, series]) => {
        if (!active) return;
        quotas = summary.quotas;
        points = series.points;
      })
      .catch((e) => {
        if (!active) return;
        error = parseError(e);
      });
    return () => { active = false; };
  });

  const primary = $derived(quotas.find((x) => x.quota_id === quotaId) ?? null);
  function fmtQuota(value: number | null): string {
    return fmtSmart(value == null ? null : primary?.unit === 'milli_requests' ? value / 1000 : value);
  }
  const pct = $derived.by(() => {
    if (!primary || primary.limit_value == null || primary.limit_value <= 0 || primary.used == null) {
      return null;
    }
    return Math.min(100, Math.max(0, (primary.used / primary.limit_value) * 100));
  });

  function fmtTime(ms: number): string {
    try {
      return new Intl.DateTimeFormat(i18n.locale, { timeZone: timezone, dateStyle: 'short', timeStyle: 'short' }).format(
        new Date(ms),
      );
    } catch {
      return '';
    }
  }

  // 趋势：used 随天变化的折线（单点显示圆点）。viewBox 100×32。
  const spark = $derived.by(() => {
    const vals = points.map((p) => p.used).filter((v): v is number => v != null);
    if (vals.length === 0) return null;
    const max = Math.max(...vals);
    const min = Math.min(...vals);
    const range = max - min || 1;
    const n = vals.length;
    const pts = vals.map((v, i) => {
      const x = n === 1 ? 50 : (i / (n - 1)) * 100;
      const y = 30 - ((v - min) / range) * 28;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    return { pts: pts.join(' '), single: n === 1, last: pts[pts.length - 1] };
  });
</script>

<div class="quota-card" class:dark={isDark}>
  <div class="head">
    <span class="agent">{agentLabel}</span>
    <span class="unit">{primary?.unit === 'requests' || primary?.unit === 'milli_requests' ? t('quota.premiumRequests') : quotaId}</span>
  </div>

  {#if error}
    <p class="err">{error}</p>
  {:else if !primary}
    <p class="empty">{t('quota.empty')}</p>
  {:else}
    <div class="figure">
      <span class="used">{fmtQuota(primary.used)}</span>
      {#if primary.limit_value != null && primary.limit_value > 0}
        <span class="limit">/ {fmtQuota(primary.limit_value)}</span>
      {/if}
      <span class="usedlabel">{t('quota.usedLabel')}</span>
    </div>
    {#if pct != null}
      <div class="bar" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
        <div class="fill" style={`width:${pct}%`}></div>
      </div>
    {/if}
    <div class="meta">
      {#if primary.remaining != null}<span>{t('quota.remaining', { n: fmtQuota(primary.remaining) })}</span>{/if}
      <span class="at">{t('quota.snapshotAt', { time: fmtTime(primary.observed_at_ms) })}</span>
    </div>

    {#if spark}
      <div class="trend">
        <span class="trend-title">{t('quota.trend')}</span>
        <svg viewBox="0 0 100 32" preserveAspectRatio="none" class="spark" aria-hidden="true">
          {#if spark.single}
            <circle cx={spark.last.split(',')[0]} cy={spark.last.split(',')[1]} r="2" class="dot" />
          {:else}
            <polyline points={spark.pts} fill="none" class="line" />
          {/if}
        </svg>
      </div>
    {/if}

    <p class="note">{t('quota.note')}</p>
  {/if}
</div>

<style>
  .quota-card {
    border: 1px solid var(--border, #e2e3e8);
    border-radius: 10px;
    padding: 14px 16px;
    background: var(--panel-bg, #fff);
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 220px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }
  .agent {
    font-weight: 600;
    font-size: 0.95rem;
  }
  .unit {
    font-size: 0.78rem;
    opacity: 0.7;
  }
  .figure {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .used {
    font-size: 1.8rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .limit {
    font-size: 1rem;
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }
  .usedlabel {
    font-size: 0.78rem;
    opacity: 0.7;
    margin-left: 2px;
  }
  .bar {
    height: 8px;
    border-radius: 4px;
    background: var(--track, #eceef3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: linear-gradient(90deg, #4f8cff, #8b5cf6);
    border-radius: 4px;
  }
  .meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 0.78rem;
    opacity: 0.75;
  }
  .trend {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .trend-title {
    font-size: 0.74rem;
    opacity: 0.65;
  }
  .spark {
    width: 100%;
    height: 32px;
  }
  .line {
    stroke: #8b5cf6;
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }
  .dot {
    fill: #8b5cf6;
  }
  .note {
    font-size: 0.72rem;
    opacity: 0.6;
    margin: 0;
  }
  .empty,
  .err {
    font-size: 0.82rem;
    opacity: 0.7;
    margin: 4px 0;
  }
  .err {
    color: #d14343;
  }
  .quota-card.dark {
    --border: #333844;
    --panel-bg: #1c1f26;
    --track: #2a2e38;
  }
</style>
