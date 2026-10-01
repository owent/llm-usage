<script lang="ts">
  /**
   * F2 费用面板：按发生时价估算 / 来源记录金额 / 按当前价格模拟三列分币种展示。
   * 不同币种只分组小计不合并（E8）；未计价与部分计价如实标注，不补零。
   * 面板仅在设置启用费用估算后由父级渲染。
   */
  import { api, parseError } from '../lib/api';
  import type { CostSummaryDto, SummaryQuery } from '../lib/api';
  import { i18n, t } from '../lib/i18n.svelte';

  let {
    query,
    refreshKey = 0,
  }: {
    query: SummaryQuery;
    /** 父级刷新/重算后递增以重新加载。 */
    refreshKey?: number;
  } = $props();

  let summary = $state<CostSummaryDto | null>(null);
  let error = $state('');

  $effect(() => {
    const q = query;
    const key = refreshKey;
    void (key);
    error = '';
    summary = null;
    api
      .costSummary(q)
      .then((r) => {
        summary = r;
      })
      .catch((e) => {
        error = parseError(e);
      });
  });

  /** 最小货币单位 → 本地化金额字符串（整数最小单位 / 100）。 */
  function fmtAmount(currency: string, minor: number): string {
    const value = minor / 100;
    try {
      return new Intl.NumberFormat(i18n.locale, {
        style: 'currency',
        currency,
        currencyDisplay: 'narrowSymbol',
      }).format(value);
    } catch {
      return `${value.toFixed(2)} ${currency}`;
    }
  }

  function coverage(priced: number, known: number): string {
    if (known <= 0) return '—';
    return t('cost.coverage', { priced: fmtTokens(priced), known: fmtTokens(known) });
  }

  function fmtTokens(n: number): string {
    return new Intl.NumberFormat(i18n.locale, { notation: n >= 100_000 ? 'compact' : 'standard' }).format(n);
  }

  const hasData = $derived(
    !!summary &&
      (summary.at_time.rows.length > 0 ||
        summary.source_amounts.length > 0 ||
        summary.current_sim.rows.length > 0),
  );
</script>

<div class="cost-panel">
  {#if error}
    <p class="bad">{error}</p>
  {:else if !summary}
    <p class="hint">{t('common.loading')}</p>
  {:else if !hasData}
    <p class="hint">{t('cost.noData')}</p>
  {:else}
    {#if summary.at_time.rows.length > 0}
      <h5>{t('cost.atTime')}</h5>
      <ul>
        {#each summary.at_time.rows as row (row.currency)}
          {#if row.currency === ''}
            {#if row.unpriced_event_count > 0}
              <li class="muted">{t('cost.unpricedCount', { count: row.unpriced_event_count })}</li>
            {/if}
          {:else}
            <li>
              <span class="amount">{fmtAmount(row.currency, row.total_amount_minor)}</span>
              <span class="meta">
                {coverage(row.priced_tokens, row.known_tokens)}
                {#if row.partial_event_count > 0} · {t('cost.partialCount', { count: row.partial_event_count })}{/if}
                {#if row.fallback_event_count > 0} · {t('cost.fallbackCount', { count: row.fallback_event_count })}{/if}
              </span>
            </li>
          {/if}
        {/each}
      </ul>
      {#if Object.keys(summary.at_time.unpriced_reasons).length > 0}
        <p class="meta">
          {t('cost.unpricedReasons')}:
          {Object.entries(summary.at_time.unpriced_reasons)
            .map(([reason, count]) => `${reason} ×${count}`)
            .join(' · ')}
        </p>
      {/if}
    {/if}

    {#if summary.current_sim.rows.length > 0}
      <h5>{t('cost.currentSim')}</h5>
      <ul>
        {#each summary.current_sim.rows as row (row.currency)}
          {#if row.currency !== ''}
            <li>
              <span class="amount">{fmtAmount(row.currency, row.total_amount_minor)}</span>
              <span class="meta">{coverage(row.priced_tokens, row.known_tokens)}</span>
            </li>
          {/if}
        {/each}
      </ul>
      {#if summary.current_sim.detail_limited}
        <p class="meta">{t('cost.detailLimited')}</p>
      {/if}
    {/if}

    {#if summary.source_amounts.length > 0}
      <h5>{t('cost.sourceReported')}</h5>
      <ul>
        {#each summary.source_amounts as row (row.currency)}
          <li>
            <span class="amount">{fmtAmount(row.currency, row.total_amount_minor)}</span>
          </li>
        {/each}
      </ul>
    {/if}

    {#if summary.price_basis.length > 0}
      <p class="meta">{summary.price_basis.join(' · ')}</p>
    {/if}
  {/if}
</div>

<style>
  .cost-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-variant-numeric: tabular-nums;
  }
  h5 {
    margin: 6px 0 0;
    font-size: 0.85em;
    opacity: 0.85;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }
  .amount {
    font-weight: 600;
  }
  .meta,
  .muted {
    font-size: 0.85em;
    opacity: 0.75;
  }
  .bad {
    color: var(--danger, #c0392b);
    margin: 0;
  }
  .hint {
    opacity: 0.7;
    margin: 0;
  }
</style>
