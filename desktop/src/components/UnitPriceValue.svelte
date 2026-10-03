<script lang="ts">
  import {formatUnitPrice} from '../lib/costs';
  import {cnyToUsd,referenceFx} from '../lib/exchange-rates';
  import {i18n,t} from '../lib/i18n.svelte';
  let {currency,units}:{currency:string;units:number|null}=$props();
  const usd=$derived(cnyToUsd(currency,units));
</script>
<span>{formatUnitPrice(i18n.locale,currency,units)}</span>
{#if usd!==null}<small title={t('cost.fxHint',{date:referenceFx.date})}>≈ {formatUnitPrice(i18n.locale,'USD',usd)}</small>{/if}
<style>small{display:block;color:var(--text-muted);font-size:11px;margin-top:3px;}</style>
