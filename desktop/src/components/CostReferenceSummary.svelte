<script lang="ts">
  import type {CostSummaryDto} from '../lib/api';
  import {t} from '../lib/i18n.svelte';
  import CostAmounts from './CostAmounts.svelte';
  let {summary,error=''}:{summary:CostSummaryDto|null;error?:string}=$props();
</script>
<div class="cost-ref card" aria-label={t('cost.currentSim')} title={`${t('cost.currentSim')} · ${t('dashboard.priceReference')}`}>
    <div class="label">{t('cost.referenceShort')}</div>
    <div class="value">
      {#if error}<span class="err" title={error}>—</span>
      {:else if !summary}<span class="muted">{t('common.loading')}</span>
      {:else}<CostAmounts rows={summary.current_sim.rows} />{/if}
    </div>
    {#if summary?.current_sim.detail_limited}<small class="muted">{t('cost.detailLimited')}</small>{/if}
</div>
<style>
  .card {
    background:var(--accent-bg);
    border:1px solid var(--border);
    border-radius:9px;
    padding:12px 10px;
    min-width:0;
    max-width:100%;
    display:flex;
    flex-direction:column;
    gap:4px;
  }
  .label {
    font-size:12px;
    color:var(--text-secondary);
    white-space:nowrap;
    overflow:hidden;
    text-overflow:ellipsis;
  }
  .value {
    display:flex;
    flex-wrap:wrap;
    align-items:baseline;
    gap:2px 16px;
    font-size:19px;
    font-weight:650;
    color:var(--accent);
    font-variant-numeric:tabular-nums;
  }
  .muted {color:var(--text-muted);font-size:12px;font-weight:400;}
  .err {color:var(--danger);}
  small {font-size:11px;}
</style>
