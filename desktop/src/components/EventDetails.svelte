<script lang="ts">
  /**
   * 用量明细页（任务 F）：event_details 分页表格（每页 50 条，上/下页 + 跳转）。
   * 列：时间/Agent/模型/类别/输入/缓存命中/输出/总量/耗时（秒，1 位小数）/会话。
   * 时间格式化到秒；查询条件（Agent/模型过滤）与全局筛选联动。
   */
  import { api, parseError } from '../lib/api';
  import type { EventDetailRowDto, SummaryQuery } from '../lib/api';
  import { i18n, t, fmtPrecise } from '../lib/i18n.svelte';

  let {
    query,
    reloadKey = 0,
  }: {
    query: SummaryQuery;
    /** 用户切换/导入等不改变 query 的强制重查信号。 */
    reloadKey?: number;
  } = $props();

  const PAGE_SIZE_OPTIONS = [20, 50, 100, 200];
  let pageSize = $state(50);

  let rows = $state<EventDetailRowDto[]>([]);
  let total = $state(0);
  let page = $state(0);
  let jumpText = $state('');
  let loading = $state(false);
  let error = $state('');
  let loadToken = 0;

  const totalPages = $derived(Math.max(1, Math.ceil(total / pageSize)));

  function fmtTime(ms: number): string {
    return new Intl.DateTimeFormat(i18n.locale, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    }).format(new Date(ms));
  }

  /** 耗时：毫秒 → 秒，保留 1 位小数。 */
  function fmtSeconds(ms: string | null): string {
    if (ms === null) return '—';
    return `${(Number(ms) / 1000).toFixed(1)} s`;
  }

  async function load() {
    const token = ++loadToken;
    loading = true;
    error = '';
    try {
      const r = await api.eventDetails(query, page, pageSize);
      if (token !== loadToken) return;
      rows = r.rows;
      total = r.total;
    } catch (e) {
      if (token !== loadToken) return;
      error = parseError(e);
      rows = [];
      total = 0;
    } finally {
      if (token === loadToken) loading = false;
    }
  }

  // 查询条件/强制重查信号变化：回到第 1 页并防抖重查（声明在分页动作前，
  // 同一 flush 内先重置页码再加载，避免旧页码多拉一次）。
  let queryTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void query;
    void reloadKey;
    page = 0;
    if (queryTimer) clearTimeout(queryTimer);
    queryTimer = setTimeout(() => void load(), 300);
  });

  function go(next: number) {
    const clamped = Math.min(Math.max(0, next), totalPages - 1);
    if (clamped === page) return;
    page = clamped;
    void load();
  }

  function doJump() {
    const n = Number(jumpText.trim());
    jumpText = '';
    if (Number.isInteger(n) && n >= 1) go(n - 1);
  }
</script>

<div class="details">
  {#if error}
    <p class="error">{t('common.error', { message: error })}</p>
  {/if}
  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th>{t('details.time')}</th>
          <th>{t('table.agent')}</th>
          <th>{t('table.model')}</th>
          <th>{t('details.category')}</th>
          <th class="num">{t('table.input')}</th>
          <th class="num">{t('table.cacheRead')}</th>
          <th class="num">{t('table.output')}</th>
          <th class="num">{t('table.total')}</th>
          <th class="num">{t('details.duration')}</th>
          <th>{t('details.session')}</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as r (r.event_id)}
          <tr>
            <td class="time">{fmtTime(r.occurred_at_ms)}</td>
            <td>{r.agent}</td>
            <td>{r.model ?? t('common.unknown')}</td>
            <td>{r.category ?? t('common.unknown')}</td>
            <td class="num">{fmtPrecise(r.input)}</td>
            <td class="num">{fmtPrecise(r.cache_read)}</td>
            <td class="num">{fmtPrecise(r.output)}</td>
            <td class="num">{fmtPrecise(r.total)}</td>
            <td class="num">{fmtSeconds(r.duration_ms)}</td>
            <td class="session" title={r.session ?? ''}>{r.session ?? '—'}</td>
          </tr>
        {:else}
          <tr><td colspan="10" class="empty">{loading ? t('common.loading') : t('details.empty')}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div class="pager">
    <label class="size-select">
      {t('details.rowsPerPage')}
      <select
        bind:value={pageSize}
        onchange={() => {
          page = 0;
          void load();
        }}
      >
        {#each PAGE_SIZE_OPTIONS as size (size)}
          <option value={size}>{size}</option>
        {/each}
      </select>
    </label>
    <button type="button" disabled={loading || page <= 0} onclick={() => go(page - 1)}>
      {t('details.prev')}
    </button>
    <span class="page-info">
      {t('details.pageOf', { page: page + 1, pages: totalPages })}
      <span class="muted">· {t('details.totalRows', { count: fmtPrecise(total) })}</span>
    </span>
    <button type="button" disabled={loading || page >= totalPages - 1} onclick={() => go(page + 1)}>
      {t('details.next')}
    </button>
    <span class="jump">
      <input
        inputmode="numeric"
        placeholder="—"
        bind:value={jumpText}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            doJump();
          }
        }}
      />
      <button type="button" disabled={loading} onclick={doJump}>{t('details.jumpGo')}</button>
    </span>
  </div>
</div>

<style>
  .details {
    padding: 8px 0;
  }
  .error {
    color: #b3261e;
    background: #fdecea;
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 13px;
  }
  .table-wrap {
    overflow-x: auto;
    background: #fff;
    border: 1px solid #e6e8eb;
    border-radius: 10px;
    box-shadow: 0 1px 3px rgba(16, 24, 40, 0.06);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th,
  td {
    text-align: left;
    padding: 5px 8px;
    border-bottom: 1px solid #f0f1f3;
    white-space: nowrap;
  }
  thead th {
    color: #666;
    font-weight: 500;
    background: #fafbfc;
    position: sticky;
    top: 0;
  }
  tbody tr:hover {
    background: #f7f9fd;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .time {
    font-variant-numeric: tabular-nums;
  }
  .session {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .empty {
    text-align: center;
    color: #999;
    padding: 24px 0;
  }
  .pager {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 2px;
    font-size: 12.5px;
    flex-wrap: wrap;
  }
  .size-select {
    display: flex;
    align-items: center;
    gap: 4px;
    color: #555;
    font-size: 12px;
  }
  .size-select select {
    padding: 3px 6px;
    font-size: 12px;
  }
  .pager button {
    padding: 4px 12px;
    cursor: pointer;
    font-size: 12.5px;
    border: 1px solid #dcdfe3;
    background: #fff;
    border-radius: 6px;
    color: #444;
  }
  .pager button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .page-info {
    font-variant-numeric: tabular-nums;
    color: #444;
  }
  .muted {
    color: #888;
  }
  .jump {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .jump input {
    width: 64px;
    padding: 3px 6px;
    font-size: 12.5px;
    text-align: right;
    box-sizing: border-box;
  }
</style>
