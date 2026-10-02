<script lang="ts">
  import type {CostSummaryDto} from '../lib/api';
  import {t} from '../lib/i18n.svelte';
  import CostAmounts from './CostAmounts.svelte';
  let {summary,error=''}:{summary:CostSummaryDto|null;error?:string}=$props();
</script>
<div class="reference-summary" aria-label={t('cost.currentSim')}>
  <span class="label" title={t('dashboard.priceReference')}>{t('cost.currentSim')}</span>
  <div class="values">
    {#if error}<span class="error" title={error}>—</span>
    {:else if !summary}<span>{t('common.loading')}</span>
    {:else}<CostAmounts rows={summary.current_sim.rows} />{/if}
  </div>
  {#if summary?.current_sim.detail_limited}<small>{t('cost.detailLimited')}</small>{/if}
</div>
<style>
  .reference-summary {display:flex;align-items:baseline;flex-wrap:wrap;gap:8px 18px;border-top:1px solid var(--border-light);padding:10px 2px 2px;font-size:12px;}
  .label {color:var(--text-secondary);}
  .values {display:flex;align-items:baseline;flex-wrap:wrap;gap:4px 16px;font-weight:600;color:var(--accent);font-size:17px;}
  small {font-size:11px;color:var(--text-muted);}
  .error {color:var(--danger);}
</style>
