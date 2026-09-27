<script lang="ts">
  import { api, parseError } from '../lib/api';
  import type { EventDetailRowDto, SummaryQuery } from '../lib/api';
  import { i18n, t, fmtPrecise } from '../lib/i18n.svelte';

  let {
    query,
    reloadKey = 0,
    timezone = 'UTC',
    scopeKey = '',
  }: {
    query: SummaryQuery;
    /** 用户切换/导入等不改变 query 的强制重查信号。 */
    reloadKey?: number;
    timezone?: string;
    scopeKey?: string;
  } = $props();

  const PAGE_SIZE_OPTIONS = [20, 50, 100, 200];
  let pageSize = $state(50);

  let rows = $state<EventDetailRowDto[]>([]);
  let total = $state(0);
  let page = $state(0);
  let jumpText = $state('');
  /** 初始 true：挂载即骨架占位，首次请求返回后填充。 */
  let loading = $state(true);
  let error = $state('');
  let loadToken = 0;

  const totalPages = $derived(Math.max(1, Math.ceil(total / pageSize)));
  /** 首屏加载（尚无任何行）时显示骨架矩形而非空表。 */
  const showSkeleton = $derived(loading && rows.length === 0 && error === '');

  function fmtTime(ms: number): string {
    return new Intl.DateTimeFormat(i18n.locale, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
      timeZone: timezone,
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
      // 只请求当前页：page/pageSize 原样传给后端，后端做 LIMIT/OFFSET。
      const r = await api.eventDetails(query, page, pageSize);
      if (token !== loadToken) return;
      total = r.total;
      const lastPage = Math.max(0, Math.ceil(r.total / pageSize) - 1);
      if (page > lastPage) {
        page = lastPage;
        await load();
        return;
      }
      rows = r.rows;
    } catch (e) {
      if (token !== loadToken) return;
      error = parseError(e);
      rows = [];
      total = 0;
    } finally {
      if (token === loadToken) loading = false;
    }
  }

  // 查询范围变化时回到第 1 页；后台刷新保留页码，数据缩减时回到最后一页。
  // queryKey 为查询字段的稳定序列化：父级每次轮询重建 query 对象（身份变化）
  // 但字段值不变时不重查，只有真正的过滤/范围变化才重新请求。
  const queryKey = $derived(
    JSON.stringify([
      query.first_day,
      query.last_day,
      query.granularity,
      query.agents,
      query.providers,
      query.models,
    ])
  );
  let previousScope = '';
  $effect(() => {
    const scope = JSON.stringify([queryKey, scopeKey, timezone]);
    void reloadKey;
    if (scope !== previousScope) {
      previousScope = scope;
      page = 0;
      rows = [];
      total = 0;
      loading = true;
    }
    ++loadToken;
    const timer = setTimeout(() => void load(), 200);
    return () => { ++loadToken; clearTimeout(timer); };
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
  {#if showSkeleton}
    <!-- 首屏骨架：灰色矩形占位，请求返回后替换为表格。 -->
    <div class="skeleton" aria-busy="true">
      {#each Array.from({ length: 10 }) as _, i (i)}
        <div class="sk-row" style:width="{96 - ((i * 7) % 30)}%"></div>
      {/each}
    </div>
  {:else}
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
  {/if}

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
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 13px;
  }
  .table-wrap {
    overflow-x: auto;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: var(--shadow);
  }
  /* 首屏骨架：灰色矩形占位（微光动画），请求返回后替换为表格。 */
  .skeleton {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: var(--shadow);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sk-row {
    height: 14px;
    border-radius: 4px;
    background: linear-gradient(90deg, var(--bg-skeleton) 25%, var(--bg-hover) 45%, var(--bg-skeleton) 65%);
    background-size: 200% 100%;
    animation: sk-shimmer 1.3s ease-in-out infinite;
  }
  @keyframes sk-shimmer {
    0% {
      background-position: 120% 0;
    }
    100% {
      background-position: -80% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sk-row {
      animation: none;
    }
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
    border-bottom: 1px solid var(--border-light);
    white-space: nowrap;
  }
  thead th {
    color: var(--text-secondary);
    font-weight: 500;
    background: var(--bg-card-hover);
    position: sticky;
    top: 0;
  }
  tbody tr:hover {
    background: var(--bg-hover);
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
    color: var(--text-muted);
    padding: 24px 0;
  }
  .pager {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 2px;
    font-size: 13px;
    flex-wrap: wrap;
  }
  .size-select {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text-secondary);
    font-size: 12px;
  }
  .size-select select {
    padding: 3px 6px;
    font-size: 12px;
  }
  .pager button {
    padding: 4px 12px;
    cursor: pointer;
    font-size: 13px;
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .pager button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .page-info {
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }
  .muted {
    color: var(--text-muted);
  }
  .jump {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .jump input {
    width: 64px;
    padding: 3px 6px;
    font-size: 13px;
    text-align: right;
    box-sizing: border-box;
  }
</style>
