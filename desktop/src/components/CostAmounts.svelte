<script lang="ts">
  import type { CostCurrencyRowDto } from '../lib/api';
  import { pricedAmounts, formatAmount } from '../lib/costs';
  import { i18n,t } from '../lib/i18n.svelte';
  let {rows=[]}:{rows?:CostCurrencyRowDto[]}=$props();
  const priced=$derived(pricedAmounts(rows));
  const unpriced=$derived(rows.reduce((sum,r)=>sum+r.unpriced_event_count,0));
</script>
{#if !priced.length}<span>—</span>{/if}
{#each priced as row (row.currency)}
  <div class="money" title={t('cost.coverage',{priced:row.priced_tokens,known:row.known_tokens})}>
    <span>{row.currency} {formatAmount(i18n.locale,row.currency,row.total_amount_minor)}</span>
    {#if row.partial_event_count}<small>{t('cost.partialCount',{count:row.partial_event_count})}</small>{/if}
  </div>
{/each}
{#if unpriced}<small>{t('cost.unpricedCount',{count:unpriced})}</small>{/if}
<style>
  .money { white-space: nowrap; font-variant-numeric:tabular-nums; }
  small { display:block; font-size:11px; color:var(--text-muted); margin-top:3px; white-space:normal; }
</style>
