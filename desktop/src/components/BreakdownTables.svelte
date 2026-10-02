<script lang="ts">
  import type { MetricSumsDto, CostSummaryDto } from '../lib/api';
  import CostAmounts from './CostAmounts.svelte';
  import { t, fmtNumber } from '../lib/i18n.svelte';

  let {
    models = [],
    agents = [],
    kind = 'both',
    totals = null,
    costs = null,
    pricing = false,
  }: {
    models?: { provider: string | null; model: string | null; sums: MetricSumsDto }[];
    agents?: { agent: string; sums: MetricSumsDto }[];
    kind?: 'both' | 'model' | 'agent';
    totals?: MetricSumsDto | null;
    costs?: CostSummaryDto | null;
    pricing?: boolean;
  } = $props();

  function unknownNote(sums: MetricSumsDto): string {
    const unknown = sums.input_unknown_count + sums.output_unknown_count;
    return unknown > 0 ? t('table.unknownFields', { count: unknown }) : '';
  }
  const key=(model:string|null)=>model?.toLowerCase().startsWith('claude-') ? model.toLowerCase().replaceAll('.','-') : (model??'').toLowerCase();
  function costOf(provider:string|null,model:string|null) {
    return costs?.models?.find((r)=>r.provider.toLowerCase()===(provider??'').toLowerCase() && key(r.model)===key(model));
  }
</script>

<div class="tables" class:single={kind !== 'both'}>
  {#if kind === 'both' || kind === 'model'}
    <section>
      {#if kind === 'both'}<h3>{t('table.model')}</h3>{/if}
      <table>
        <thead>
          <tr>
            <th>{t('table.model')}</th>
            <th>{t('table.calls')}</th>
            <th>{t('table.input')}</th>
            <th>{t('table.output')}</th>
            <th>{t('table.total')}</th>
            <th>{t('table.cacheRead')}</th>
            {#if pricing}<th class="num">{t('cost.currentSim')}</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each models as m (`${m.provider ?? ''}/${m.model ?? ''}`)}
            <tr>
              <td><strong>{m.model ?? t('common.unknown')}</strong><small>{m.provider ?? t('common.unknown')}</small></td>
              <td class="num" title={m.sums.call_count === 0 && m.sums.event_count > 0 ? t('dashboard.callsUnknown') : ''}>{fmtNumber(m.sums.call_count === 0 && m.sums.event_count > 0 ? null : m.sums.call_count)}</td>
              <td class="num">{fmtNumber(m.sums.input_total_known)}</td>
              <td class="num">{fmtNumber(m.sums.output_total_known)}</td>
              <td class="num">{fmtNumber(m.sums.total_tokens_known)}</td>
              <td class="num">{fmtNumber(m.sums.cache_read_known)}</td>
              {#if pricing}
                <td class="num"><CostAmounts rows={costOf(m.provider,m.model)?.current_sim} /></td>
              {/if}
            </tr>
            {#if unknownNote(m.sums)}
              <tr class="note-row"><td colspan={pricing ? 7 : 6}>{unknownNote(m.sums)}</td></tr>
            {/if}
          {/each}
        </tbody>
        {#if totals}
          <tfoot><tr>
            <th>{t('dashboard.subtotal')}</th><td class="num">{fmtNumber(totals.call_count)}</td>
            <td class="num">{fmtNumber(totals.input_total_known)}</td><td class="num">{fmtNumber(totals.output_total_known)}</td>
            <td class="num">{fmtNumber(totals.total_tokens_known)}</td><td class="num">{fmtNumber(totals.cache_read_known)}</td>
            {#if pricing}<td class="num"><CostAmounts rows={costs?.current_sim.rows} /></td>{/if}
          </tr></tfoot>
        {/if}
      </table>
      {#if pricing}<p class="reference">{t('dashboard.costCurveHint')}</p>{/if}
    </section>
  {/if}
  {#if kind === 'both' || kind === 'agent'}
    <section>
      {#if kind === 'both'}<h3>{t('table.agent')}</h3>{/if}
      <table>
        <thead>
          <tr>
            <th>{t('table.agent')}</th>
            <th>{t('table.calls')}</th>
            <th>{t('table.input')}</th>
            <th>{t('table.output')}</th>
            <th>{t('table.total')}</th>
          </tr>
        </thead>
        <tbody>
          {#each agents as a (a.agent)}
            <tr>
              <td>{a.agent}</td>
              <td class="num" title={a.sums.call_count === 0 && a.sums.event_count > 0 ? t('dashboard.callsUnknown') : ''}>{fmtNumber(a.sums.call_count === 0 && a.sums.event_count > 0 ? null : a.sums.call_count)}</td>
              <td class="num">{fmtNumber(a.sums.input_total_known)}</td>
              <td class="num">{fmtNumber(a.sums.output_total_known)}</td>
              <td class="num">{fmtNumber(a.sums.total_tokens_known)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/if}
</div>

<style>
  section { min-width: 0; overflow-x: auto; }
  td strong { font-weight: 550; }
  td small { display: block; font-size: 11px; color: var(--text-muted); margin-top: 3px; }
  .tables {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    padding: 8px 0;
  }
  .tables.single {
    grid-template-columns: 1fr;
  }
  @media (max-width: 900px) {
    .tables {
      grid-template-columns: 1fr;
    }
  }
  h3 {
    font-size: 14px;
    margin: 4px 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th,
  td {
    text-align: left;
    padding: 10px 8px;
    border-bottom: 1px solid var(--border-light);
    white-space: nowrap;
  }
  th {
    color: var(--text-secondary);
    font-weight: 500;
    white-space: normal;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .note-row td {
    color: var(--warning);
    font-size: 12px;
    white-space: normal;
  }
  td:first-child { white-space:normal; overflow-wrap:anywhere; min-width:140px; }
  tfoot { background:var(--bg-hover); font-weight:600; }
  .reference { color:var(--text-muted); font-size:12px; margin:8px 0 0; }
</style>
