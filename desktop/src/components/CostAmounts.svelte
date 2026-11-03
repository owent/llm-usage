<script lang="ts">
  import type { CostCurrencyRowDto } from '../lib/api';
  import { pricedAmounts, formatAmount, formatCostRange } from '../lib/costs';
  import { i18n,t } from '../lib/i18n.svelte';
  import {cnyToUsd,referenceFx} from '../lib/exchange-rates';
  let {rows=[],compact=false}:{rows?:CostCurrencyRowDto[];compact?:boolean}=$props();
  const priced=$derived(pricedAmounts(rows));
  const unpriced=$derived(rows.reduce((sum,r)=>sum+r.unpriced_event_count,0));
</script>
{#if !priced.length}<span>—</span>{/if}
{#each priced as row (row.currency)}
  {@const usd=cnyToUsd(row.currency,row.total_amount_minor)}
  {@const usdUpper=cnyToUsd(row.currency,row.upper_amount_minor??null)}
  {@const ranged=typeof row.upper_amount_minor==='number' && Number.isFinite(row.upper_amount_minor)}
  <div class="money" class:compact title={[`${row.currency} ${formatCostRange(i18n.locale,row)}`,t('cost.coverage',{priced:row.priced_tokens,known:row.known_tokens}),row.partial_event_count?t('cost.partialCount',{count:row.partial_event_count}):''].filter(Boolean).join(' · ')}>
    <span class:ranged={compact&&ranged}>
      {#if compact&&ranged}
        <span class="bound">{row.currency} {formatAmount(i18n.locale,row.currency,row.total_amount_minor)}</span>
        <span class="bound"> – {formatAmount(i18n.locale,row.currency,row.upper_amount_minor)}</span>
      {:else}{row.currency} {formatCostRange(i18n.locale,row)}{/if}
    </span>
    {#if row.substitute_models?.length}<small title={t('cost.substituteHint',{models:row.substitute_models.join(', ')})}>{compact?t('cost.substituteShort'):t('cost.substituteHint',{models:row.substitute_models.join(', ')})}</small>{/if}
    {#if usd!==null}<small title={`${t('cost.fxHint',{date:referenceFx.date})} · ≈ USD ${formatCostRange(i18n.locale,{currency:'USD',total_amount_minor:usd,upper_amount_minor:usdUpper})}`}>
      {#if compact&&usdUpper!==null}<span class="converted-range"><span>≈ USD {formatAmount(i18n.locale,'USD',usd)}</span><span> – {formatAmount(i18n.locale,'USD',usdUpper)}</span></span>
      {:else}≈ USD {formatCostRange(i18n.locale,{currency:'USD',total_amount_minor:usd,upper_amount_minor:usdUpper})}{/if}
    </small>{/if}
    {#if row.upper_amount_minor!=null}<small title={t('cost.tierRange')}>{t('cost.tierRange')}</small>{/if}
    {#if !compact && row.aggregate_event_count}<small>{t('cost.aggregateCount',{count:row.aggregate_event_count})}</small>{/if}
    {#if !compact && row.partial_event_count}<small title={t('cost.partialHint')}>{t('cost.partialCount',{count:row.partial_event_count})}</small>{/if}
  </div>
{/each}
{#if !compact && unpriced}<small>{t('cost.unpricedCount',{count:unpriced})}</small>{/if}
<style>
  .money { min-width:0; max-width:100%; overflow-wrap:anywhere; font-variant-numeric:tabular-nums; }
  .compact {display:flex;align-items:baseline;flex-wrap:wrap;gap:0 6px;}
  .compact > span {white-space:nowrap;max-width:100%;overflow:hidden;text-overflow:ellipsis;}
  .compact > span.ranged {flex-basis:100%;white-space:normal;overflow:visible;text-overflow:clip;}
  .ranged .bound,.converted-range > span {display:block;white-space:normal;overflow-wrap:anywhere;line-height:1.35;}
  .compact small {flex-basis:100%;min-width:0;max-width:100%;margin:0;white-space:normal;overflow:visible;text-overflow:clip;overflow-wrap:anywhere;line-height:1.35;}
  small { display:block; font-size:11px; color:var(--text-muted); margin-top:3px; white-space:normal; }
</style>
