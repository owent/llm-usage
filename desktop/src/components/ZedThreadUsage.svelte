<script lang="ts">
  import type { ZedThreadUsageDto } from '../lib/api';
  import { fmtNumber, i18n, t } from '../lib/i18n.svelte';
  import { createZedTimeFormatter, formatZedInterval } from '../lib/zed-usage';

  let { rows, timezone, error }: { rows: ZedThreadUsageDto[]; timezone: string; error: string } = $props();
  const formatter = $derived(createZedTimeFormatter(i18n.locale, timezone));
  const columns = [
    ['output_total', 'cards.output'],
    ['input_total', 'cards.input'],
    ['input_uncached', 'cards.cacheMiss'],
    ['input_cache_read', 'cards.cacheRead'],
    ['input_cache_write', 'cards.cacheWrite'],
    ['total_tokens', 'cards.total'],
    ['observed_turns', 'zedUsage.turns'],
  ] as const;
</script>

<section class="zed-thread-usage" aria-labelledby="zed-thread-usage-title">
  <h2 id="zed-thread-usage-title">{t('zedUsage.title')}</h2>
  {#if error}
    <p class="error" role="alert">{t('common.error', { message: error })}</p>
  {:else}
    <p>{t('zedUsage.hint')}</p>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to scroll the wide table.) -->
    <div class="table-scroll" role="region" aria-label={t('zedUsage.interval')} tabindex="0">
      <table>
        <thead>
          <tr>
            <th scope="col">{t('zedUsage.interval')}</th>
            {#each columns as [field, label] (field)}
              <th scope="col">{t(label)}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each rows as row (row.id)}
            <tr>
              <th scope="row">{formatZedInterval(row.start_ms, row.end_ms, formatter)}</th>
              {#each columns as [field] (field)}
                <td>{fmtNumber(row[field])}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .zed-thread-usage { margin-block: 0 16px; padding: 12px 16px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-card); }
  h2 { margin: 0; font-size: 16px; }
  p { margin: 6px 0 10px; color: var(--text-muted); font-size: 12px; }
  p.error { color: var(--danger); background: var(--danger-bg); border-radius: 6px; padding: 8px 12px; font-size: 13px; margin: 8px 0 0; }
  .table-scroll { max-width: 100%; max-height: 260px; overflow: auto; }
  .table-scroll:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; font-variant-numeric: tabular-nums; }
  thead th { position: sticky; top: 0; background: var(--bg-card); }
  th, td { padding: 6px 8px; border-bottom: 1px solid var(--border); text-align: right; white-space: nowrap; }
  th:first-child { text-align: left; }
  tbody th[scope="row"] { font-weight: 400; color: var(--text); }
  tbody tr:last-child th, tbody tr:last-child td { border-bottom: 0; }
</style>
