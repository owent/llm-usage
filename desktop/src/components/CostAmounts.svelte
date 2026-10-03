<script lang="ts">
  import type { CostCurrencyRowDto } from '../lib/api';
  import { pricedAmounts, formatAmount } from '../lib/costs';
  import { i18n,t } from '../lib/i18n.svelte';
  import {cnyToUsd,referenceFx} from '../lib/exchange-rates';
  let {rows=[],compact=false}:{rows?:CostCurrencyRowDto[];compact?:boolean}=$props();
  const priced=$derived(pricedAmounts(rows));
  const unpriced=$derived(rows.reduce((sum,r)=>sum+r.unpriced_event_count,0));
</script>
{#if !priced.length}<span>—</span>{/if}
{#each priced as row (row.currency)}
  {@const usd=cnyToUsd(row.currency,row.total_amount_minor)}
  <div class="money" class:compact title={[`${row.currency} ${formatAmount(i18n.locale,row.currency,row.total_amount_minor)}`,t('cost.coverage',{priced:row.priced_tokens,known:row.known_tokens}),row.partial_event_count?t('cost.partialCount',{count:row.partial_event_count}):''].filter(Boolean).join(' · ')}>
    <span>{row.currency} {formatAmount(i18n.locale,row.currency,row.total_amount_minor)}</span>
    {#if usd!==null}<small title={t('cost.fxHint',{date:referenceFx.date})}>≈ USD {formatAmount(i18n.locale,'USD',usd)}</small>{/if}
    {#if !compact && row.partial_event_count}<small title={t('cost.partialHint')}>{t('cost.partialCount',{count:row.partial_event_count})}</small>{/if}
  </div>
{/each}
{#if !compact && unpriced}<small>{t('cost.unpricedCount',{count:unpriced})}</small>{/if}
<style>
  .money { min-width:0; overflow-wrap:anywhere; font-variant-numeric:tabular-nums; }
  .compact {display:flex;align-items:baseline;flex-wrap:wrap;gap:0 6px;}
  .compact > span {white-space:nowrap;max-width:100%;overflow:hidden;text-overflow:ellipsis;}
  .compact small {margin:0;white-space:nowrap;}
  small { display:block; font-size:11px; color:var(--text-muted); margin-top:3px; white-space:normal; }
</style>
