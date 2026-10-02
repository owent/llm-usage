<script lang="ts">
  import type {CostSummaryDto} from '../lib/api';
  import {i18n,t} from '../lib/i18n.svelte';
  import {formatAmount} from '../lib/costs';
  import UnitPrices from './UnitPrices.svelte';
  let {summary=null,error=''}:{summary?:CostSummaryDto|null;error?:string}=$props();
  function tokens(n:number) {return new Intl.NumberFormat(i18n.locale,{notation:n>=100000?'compact':'standard'}).format(n);}
</script>
<div class="cost-panel">
  <p class="meta">{t('dashboard.priceReference')}</p>
  {#if error}<p class="bad">{error}</p>
  {:else if !summary}<p class="meta">{t('common.loading')}</p>
  {:else}
    <ul>
      {#each summary.current_sim.rows as row (row.currency)}
        {#if row.currency && row.priced_event_count}
          <li><span class="amount">{row.currency} {formatAmount(i18n.locale,row.currency,row.total_amount_minor)}</span>
            <span class="meta">{t('cost.coverage',{priced:tokens(row.priced_tokens),known:tokens(row.known_tokens)})}
              {#if row.partial_event_count} · {t('cost.partialCount',{count:row.partial_event_count})}{/if}
              {#if row.fallback_event_count} · {t('cost.fallbackCount',{count:row.fallback_event_count})}{/if}
            </span>
          </li>
        {/if}
        {#if row.unpriced_event_count}<li class="meta">{t('cost.unpricedCount',{count:row.unpriced_event_count})}</li>{/if}
      {:else}<li class="meta">{t('cost.noData')}</li>{/each}
    </ul>
    {#if Object.keys(summary.current_sim.unpriced_reasons).length}
      <p class="meta">{t('cost.unpricedReasons')}: {Object.entries(summary.current_sim.unpriced_reasons).map(([reason,count])=>`${reason} ×${count}`).join(' · ')}</p>
    {/if}
    {#if summary.current_sim.detail_limited}<p class="meta">{t('cost.detailLimited')}</p>{/if}
    {#if summary.models.length}<UnitPrices {summary} />{/if}
  {/if}
</div>
<style>
  .cost-panel {display:flex;flex-direction:column;gap:6px;font-variant-numeric:tabular-nums;}
  ul {margin:0;padding:0;list-style:none;display:flex;flex-direction:column;gap:4px;}
  li {display:flex;align-items:baseline;flex-wrap:wrap;gap:10px;}
  .amount {font-weight:650;font-size:20px;}
  .meta {font-size:12px;color:var(--text-muted);margin:0;}
  .bad {color:var(--danger);margin:0;}
</style>
