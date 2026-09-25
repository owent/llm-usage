<script lang="ts">
  import type { MetricSumsDto } from '../lib/api';
  import { t, fmtNumber } from '../lib/i18n.svelte';

  let {
    models,
    agents,
  }: {
    models: { provider: string | null; model: string | null; sums: MetricSumsDto }[];
    agents: { agent: string; sums: MetricSumsDto }[];
  } = $props();

  function unknownNote(sums: MetricSumsDto): string {
    const unknown = sums.input_unknown_count + sums.output_unknown_count;
    return unknown > 0 ? t('table.unknownFields', { count: unknown }) : '';
  }
</script>

<div class="tables">
  <section>
    <h3>{t('table.model')}</h3>
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
            <td>{m.model ?? t('common.unknown')}</td>
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
  <section>
    <h3>{t('table.agent')}</h3>
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
</div>

<style>
  .tables {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    padding: 8px 0;
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
    padding: 5px 8px;
    border-bottom: 1px solid #eee;
  }
  th {
    color: #666;
    font-weight: 500;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .note-row td {
    color: #8a6d1a;
    font-size: 12px;
  }
</style>
