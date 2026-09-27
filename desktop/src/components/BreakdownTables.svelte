<script lang="ts">
  import type { MetricSumsDto } from '../lib/api';
  import { t, fmtNumber } from '../lib/i18n.svelte';

  let {
    models = [],
    agents = [],
    kind = 'both',
  }: {
    models?: { provider: string | null; model: string | null; sums: MetricSumsDto }[];
    agents?: { agent: string; sums: MetricSumsDto }[];
    kind?: 'both' | 'model' | 'agent';
  } = $props();

  function unknownNote(sums: MetricSumsDto): string {
    const unknown = sums.input_unknown_count + sums.output_unknown_count;
    return unknown > 0 ? t('table.unknownFields', { count: unknown }) : '';
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
          </tr>
        </thead>
        <tbody>
          {#each models as m (`${m.provider ?? ''}/${m.model ?? ''}`)}
            <tr>
              <td><strong>{m.model ?? t('common.unknown')}</strong><small>{m.provider ?? t('common.unknown')}</small></td>
              <td class="num">{fmtNumber(m.sums.call_count)}</td>
              <td class="num">{fmtNumber(m.sums.input_total_known)}</td>
              <td class="num">{fmtNumber(m.sums.output_total_known)}</td>
              <td class="num">{fmtNumber(m.sums.total_tokens_known)}</td>
              <td class="num">{fmtNumber(m.sums.cache_read_known)}</td>
            </tr>
            {#if unknownNote(m.sums)}
              <tr class="note-row"><td colspan="6">{unknownNote(m.sums)}</td></tr>
            {/if}
          {/each}
        </tbody>
      </table>
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
              <td class="num">{fmtNumber(a.sums.call_count)}</td>
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
    padding: 12px 10px;
    border-bottom: 1px solid var(--border-light);
    white-space: nowrap;
  }
  th {
    color: var(--text-secondary);
    font-weight: 500;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .note-row td {
    color: var(--warning);
    font-size: 12px;
  }
</style>
