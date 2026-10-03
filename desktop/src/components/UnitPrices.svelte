<script lang="ts">
  import type {CostSummaryDto} from '../lib/api';
  import UnitPriceValue from './UnitPriceValue.svelte';
  import {referenceFx} from '../lib/exchange-rates';
  import {i18n,t} from '../lib/i18n.svelte';
  import {unpricedReasonKey} from '../lib/costs';
  let {summary}:{summary:CostSummaryDto}=$props();
</script>
<details class="unit-prices">
  <summary>{t('cost.unitPrices')}</summary>
  <p class="unit-hint">{t('cost.unitPriceHint')}</p>
  {#if summary.models.some(model=>model.unit_prices?.some(price=>price.currency==='CNY'))}
    <p class="unit-hint">{t('cost.fxHint',{date:referenceFx.date})} <a href={referenceFx.source} target="_blank" rel="noreferrer">ECB</a></p>
  {/if}
  <div class="price-scroll">
    <table>
      <thead><tr><th>{t('table.model')}</th><th>{t('table.input')}</th><th>{t('table.output')}</th><th>{t('table.cacheRead')}</th><th>{t('cost.cacheWrite5m')}</th><th>{t('cost.cacheWrite1h')}</th></tr></thead>
      <tbody>
        {#each summary.models as model (`${model.provider}/${model.model}`)}
          {#each model.unit_prices ?? [] as price (price.price_id)}
            <tr>
              <th class="model">{model.model || t('common.unknown')}
                <small>{price.provider_id} · {price.region}/{price.channel} · {price.service_tier}</small>
                <small>{t('cost.contextTier',{count:price.context_threshold_tokens.toLocaleString(i18n.locale)})}</small>
                <small class="basis" title={price.price_id}>{price.snapshot_id}{#if price.model!==model.model} · {price.model}{/if}</small>
              </th>
              <td><UnitPriceValue currency={price.currency} units={price.input_per_mtok_hundredths} /></td>
              <td><UnitPriceValue currency={price.currency} units={price.output_per_mtok_hundredths} /></td>
              <td><UnitPriceValue currency={price.currency} units={price.cache_read_per_mtok_hundredths} /></td>
              <td><UnitPriceValue currency={price.currency} units={price.cache_write_5m_per_mtok_hundredths} /></td>
              <td><UnitPriceValue currency={price.currency} units={price.cache_write_1h_per_mtok_hundredths} /></td>
            </tr>
          {:else}
            <tr><th class="model">{model.model || t('common.unknown')}
              {#each model.reference_models??[] as reference}{#if reference!==model.model}<small>→ {reference}</small>{/if}{/each}
            </th><td colspan="5" class="missing">{t('cost.noUnitPrice')}
              {#each Object.entries(model.unpriced_reasons??{}) as [reason,count]}<small>{t(unpricedReasonKey(reason))} ×{count}</small>{/each}
            </td></tr>
          {/each}
        {/each}
      </tbody>
    </table>
  </div>
</details>
<style>
  details {font-size:12px;color:var(--text-secondary);}
  summary {cursor:pointer;width:fit-content;color:var(--accent);padding:5px 0;}
  .unit-hint {font-size:12px;margin:4px 0 8px;color:var(--text-muted);}
  .price-scroll {overflow-x:auto;max-width:100%;}
  table {width:100%;border-collapse:collapse;font-size:12px;font-variant-numeric:tabular-nums;}
  th,td {padding:8px;text-align:right;border-bottom:1px solid var(--border-light);white-space:nowrap;vertical-align:top;}
  thead th {font-weight:500;white-space:normal;}
  .model {text-align:left;white-space:normal;overflow-wrap:anywhere;min-width:160px;font-weight:550;}
  small {display:block;font-weight:400;font-size:11px;color:var(--text-muted);margin-top:3px;}
  .basis {font-size:10px;}
  .missing {color:var(--text-muted);text-align:left;white-space:normal;}
</style>
