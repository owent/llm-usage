<script lang="ts">
  import { onMount } from 'svelte';
  import { api, parseError } from './lib/api';
  import type { AppSettings, SummaryDto, SourceDto, RefreshStateDto } from './lib/api';
  import { i18n, initLocale, setLocale, t } from './lib/i18n.svelte';
  import TrendChart from './components/TrendChart.svelte';
  import TodayHourly from './components/TodayHourly.svelte';
  import UsageHeatmap from './components/UsageHeatmap.svelte';
  import SummaryCards from './components/SummaryCards.svelte';
  import BreakdownTables from './components/BreakdownTables.svelte';
  import SourceList from './components/SourceList.svelte';
  import SettingsPanel from './components/SettingsPanel.svelte';

  type Tab = 'overview' | 'trend' | 'sources' | 'settings';

  let tab = $state<Tab>('overview');
  let settings = $state<AppSettings | null>(null);
  let summary = $state<SummaryDto | null>(null);
  let sources = $state<SourceDto[]>([]);
  let refresh = $state<RefreshStateDto | null>(null);
  let loadError = $state('');
  let queryError = $state('');
  let granularity = $state<'day' | 'week' | 'month'>('day');
  let quickDays = $state(30);
  let agentFilter = $state('');
  let modelFilter = $state('');

  function isoDay(d: Date): string {
    return d.toISOString().slice(0, 10);
  }

  const query = $derived.by(() => {
    const last = new Date();
    const first = new Date(last.getTime() - (quickDays - 1) * 86_400_000);
    return {
      first_day: isoDay(first),
      last_day: isoDay(last),
      granularity,
      agents: agentFilter ? [agentFilter] : [],
      providers: [],
      models: modelFilter ? [modelFilter] : [],
    };
  });

  const agentsAvailable = $derived(
    Array.from(new Set((summary?.agents ?? []).map((a) => a.agent))).sort()
  );
  const modelsAvailable = $derived(
    Array.from(new Set((summary?.models ?? []).map((m) => m.model ?? '__unknown__'))).sort()
  );

  async function loadSettings() {
    try {
      settings = await api.getSettings();
      initLocale(settings.language);
    } catch (e) {
      loadError = parseError(e);
    }
  }

  async function loadSummary() {
    queryError = '';
    try {
      summary = await api.summary(query);
    } catch (e) {
      queryError = parseError(e);
    }
  }

  async function loadSources() {
    try {
      const r = await api.listSources();
      sources = r.sources;
    } catch (e) {
      loadError = parseError(e);
    }
  }

  let lastLoadedRevision = $state(0);
  async function pollRefresh() {
    try {
      refresh = await api.refreshStatus();
      if (!refresh.running && refresh.last_finished_ms > 0) {
        // 刷新完成后以同修订重新查询（V12：刷新后 UI 用同一修订的总计/图表查询）。
        await Promise.all([loadSummary(), loadSources()]);
        lastLoadedRevision = summary?.data_revision ?? 0;
      }
    } catch {
      /* 轮询失败下次再试 */
    }
  }

  let refreshing = $state(false);
  async function manualRefresh() {
    refreshing = true;
    try {
      await api.refreshSources();
    } catch (e) {
      loadError = parseError(e);
    } finally {
      refreshing = false;
      await pollRefresh();
    }
  }

  onMount(() => {
    void (async () => {
      await loadSettings();
      await Promise.all([loadSummary(), loadSources(), pollRefresh()]);
    })();
    const pollTimer = setInterval(() => void pollRefresh(), 3000);
    return () => clearInterval(pollTimer);
  });

  // 查询条件变化即重查（防抖 300ms）。
  let queryTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void query;
    void granularity;
    if (queryTimer) clearTimeout(queryTimer);
    queryTimer = setTimeout(() => void loadSummary(), 300);
  });

  function onSettingsSaved(next: AppSettings) {
    settings = next;
    if (next.language === 'zh-CN' || next.language === 'en') setLocale(next.language);
    void loadSummary();
  }

  const refreshLabel = $derived(
    refresh?.running
      ? t('action.refreshing')
      : refresh && refresh.last_finished_ms > 0
        ? t('refresh.status.done', {
            time: new Date(refresh.last_finished_ms).toLocaleTimeString(i18n.locale),
            added: refresh.instances.reduce((s, i) => s + i.added, 0),
            updated: refresh.instances.reduce((s, i) => s + i.updated, 0),
          })
        : t('action.refresh')
  );
</script>

<main>
  <header>
    <h1><img src="/brand/app-icon.svg" width="36" height="36" alt="" />{t('app.title')}</h1>
    <span class="subtitle">{t('app.subtitle')}</span>
    <span class="spacer"></span>
    {#if summary}
      <span class="muted">{t('cards.revision', { revision: summary.data_revision })}</span>
    {/if}
    <button class="primary" disabled={refreshing || refresh?.running} onclick={manualRefresh}>
      {refreshLabel}
    </button>
  </header>

  <nav>
    {#each [['overview', t('nav.overview')], ['trend', t('nav.trend')], ['sources', t('nav.sources')], ['settings', t('nav.settings')]] as [id, label] (id)}
      <button class:active={tab === id} onclick={() => (tab = id as Tab)}>{label}</button>
    {/each}
  </nav>

  {#if loadError}
    <p class="error">{t('common.error', { message: loadError })}</p>
  {/if}

  {#if tab === 'overview' || tab === 'trend'}
    <div class="filters">
      <label>{t('filter.range')}
        <select bind:value={quickDays}>
          <option value={7}>{t('filter.quick.7')}</option>
          <option value={30}>{t('filter.quick.30')}</option>
          <option value={365}>{t('filter.quick.365')}</option>
        </select>
      </label>
      <label>{t('filter.granularity.day')}
        <select bind:value={granularity}>
          <option value="day">{t('filter.granularity.day')}</option>
          <option value="week">{t('filter.granularity.week')}</option>
          <option value="month">{t('filter.granularity.month')}</option>
        </select>
      </label>
      <label>{t('filter.agent')}
        <select bind:value={agentFilter}>
          <option value="">{t('common.all')}</option>
          {#each agentsAvailable as a (a)}
            <option value={a}>{a}</option>
          {/each}
        </select>
      </label>
      <label>{t('filter.model')}
        <select bind:value={modelFilter}>
          <option value="">{t('common.all')}</option>
          {#each modelsAvailable as m (m)}
            <option value={m === '__unknown__' ? 'unknown' : m}>{m === '__unknown__' ? t('common.unknown') : m}</option>
          {/each}
        </select>
      </label>
    </div>
  {/if}

  {#if queryError}
    <p class="error">{t('common.error', { message: queryError })}</p>
  {/if}

  {#if tab === 'overview'}
    {#if summary}
      {#if summary.totals.call_count === 0 && summary.periods.length === 0}
        <p class="empty">{t('common.empty')}</p>
      {:else}
        <SummaryCards totals={summary.totals} excluded={summary.excluded_event_count} />
        <BreakdownTables models={summary.models} agents={summary.agents} />
        <TodayHourly hourly={summary.today_hourly} />
      {/if}
    {:else}
      <p class="muted">{t('common.loading')}</p>
    {/if}
  {:else if tab === 'trend'}
    {#if summary}
      <TrendChart periods={summary.periods} granularity={granularity} />
      <UsageHeatmap {query} />
    {:else}
      <p class="muted">{t('common.loading')}</p>
    {/if}
  {:else if tab === 'sources'}
    <SourceList {sources} onchanged={loadSources} />
  {:else if tab === 'settings'}
    {#if settings}
      <SettingsPanel settings={settings} onsaved={onSettingsSaved} />
    {/if}
  {/if}
</main>

<style>
  main {
    padding: 12px 20px 32px;
    font-family: system-ui, 'Segoe UI', sans-serif;
    color: #1c1e21;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0 10px;
    border-bottom: 1px solid #e3e5e8;
  }
  h1 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 18px;
    margin: 0;
  }
  .subtitle {
    color: #666;
    font-size: 13px;
  }
  .spacer {
    flex: 1;
  }
  nav {
    display: flex;
    gap: 4px;
    padding: 8px 0;
  }
  nav button {
    border: none;
    background: transparent;
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
  }
  nav button.active {
    background: #e8f0fe;
    color: #1a56c4;
    font-weight: 600;
  }
  .filters {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 8px 0;
    font-size: 13px;
    flex-wrap: wrap;
  }
  .filters label {
    display: flex;
    gap: 6px;
    align-items: center;
    color: #444;
  }
  select {
    padding: 3px 6px;
  }
  button.primary {
    background: #1a56c4;
    color: #fff;
    border: none;
    border-radius: 6px;
    padding: 7px 16px;
    cursor: pointer;
    font-size: 13px;
  }
  button.primary:disabled {
    background: #9db8e8;
    cursor: wait;
  }
  .error {
    color: #b3261e;
    background: #fdecea;
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 13px;
  }
  .empty {
    color: #666;
    background: #f5f6f7;
    border-radius: 8px;
    padding: 32px;
    text-align: center;
  }
  .muted {
    color: #777;
    font-size: 13px;
  }
</style>
