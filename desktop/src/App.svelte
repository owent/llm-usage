<script lang="ts">
  import { onMount } from 'svelte';
  import { api, parseError } from './lib/api';
  import type {
    AppSettings,
    HeatmapDto,
    RefreshStateDto,
    SourceDto,
    SummaryDto,
    SummaryQuery,
    UserDto,
  } from './lib/api';
  import { i18n, initLocale, setLocale, t, fmtEtaDuration, fmtSmart, fmtPercent, fmtDurationShort } from './lib/i18n.svelte';
  import { loadPanelGroup, savePanelGroup, clearPanelPage } from './lib/panels';
  import Panel from './components/Panel.svelte';
  import CallsChart from './components/CallsChart.svelte';
  import TokenChart from './components/TokenChart.svelte';
  import TodayHourly from './components/TodayHourly.svelte';
  import UsageHeatmap from './components/UsageHeatmap.svelte';
  import TodayOverview from './components/TodayOverview.svelte';
  import BreakdownTables from './components/BreakdownTables.svelte';
  import SourceList from './components/SourceList.svelte';
  import SettingsPanel from './components/SettingsPanel.svelte';
  import SharePie from './components/SharePie.svelte';
  import WeekdayBar from './components/WeekdayBar.svelte';
  import EventDetails from './components/EventDetails.svelte';

  type Tab = 'overview' | 'trend' | 'sources' | 'details' | 'settings';
  type Granularity = 'hour' | 'day' | 'week' | 'month';
  /** 时间范围（任务 C7）：近 24 小时/当天自动切小时粒度。 */
  type RangeKey = '24h' | 'today' | '7' | '30' | '365';

  let tab = $state<Tab>('overview');
  let settings = $state<AppSettings | null>(null);
  let summary = $state<SummaryDto | null>(null);
  /** 今日独立查询（任务 D8）：当日 first=last 的 summary，供今日分区使用。 */
  let todaySummary = $state<SummaryDto | null>(null);
  let sources = $state<SourceDto[]>([]);
  let refresh = $state<RefreshStateDto | null>(null);
  let loadError = $state('');
  let queryError = $state('');
  let granularity = $state<Granularity>('day');
  let rangeKey = $state<RangeKey>('30');
  let agentFilter = $state('');
  let modelFilter = $state('');

  // ---- 主题（跟随系统/亮色/暗色）：data-theme 属性 + isDark 派生（供 ECharts）。 ----
  /** 系统深色偏好实时状态（change 时更新；system 模式下系统切换立即重绘图表）。 */
  let systemDark = $state(window.matchMedia('(prefers-color-scheme: dark)').matches);

  $effect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = (e: MediaQueryListEvent) => (systemDark = e.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  });

  /** 生效主题：设置显式 light/dark 优先，其余（含未知值）回落 system。 */
  const theme = $derived(
    settings?.theme === 'light' || settings?.theme === 'dark' ? settings.theme : 'system'
  );
  /** 是否深色（显式 dark，或 system 且系统偏好深色）；传给各 ECharts 组件。 */
  const isDark = $derived(theme === 'dark' || (theme === 'system' && systemDark));

  // system = 移除 data-theme（themes.css 的 prefers-color-scheme 媒体查询接管）。
  $effect(() => {
    if (theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
  });

  // 多用户（v6）：顶栏切换 + 新建。
  let users = $state<UserDto[]>([]);
  let currentUser = $state('');
  let userSelect = $state('');
  let newUserOpen = $state(false);
  let newUserName = $state('');
  let userBusy = $state(false);
  let userError = $state('');

  // 热力图数据复用（周分布图）。
  let heatmapCells = $state<HeatmapDto['cells']>([]);
  /** 用户切换/导入等不改变 query 的强制重查信号（传给子组件）。 */
  let dataReloadKey = $state(0);

  // ---- 面板布局（任务 E10/E11）：拖拽顺序 + 显示/隐藏，按页持久化 localStorage。 ----
  type PanelGroupKey = 'overviewToday' | 'overviewHistory' | 'trendMain';
  const PANEL_GROUPS: Record<PanelGroupKey, { page: string; group: string; ids: string[] }> = {
    overviewToday: {
      page: 'overview',
      group: 'today',
      ids: [
        'today-cards',
        'today-hourly',
        'today-model-pie',
        'today-agent-pie',
        'today-model-table',
        'today-agent-table',
      ],
    },
    overviewHistory: {
      page: 'overview',
      group: 'history',
      ids: ['history-calls', 'history-tokens'],
    },
    trendMain: {
      page: 'trend',
      group: 'main',
      ids: [
        'trend-calls',
        'trend-tokens',
        'trend-heatmap',
        'trend-weekday',
        'trend-model-pie',
        'trend-agent-pie',
      ],
    },
  };

  let panelState = $state({
    overviewToday: loadPanelGroup(
      PANEL_GROUPS.overviewToday.page,
      PANEL_GROUPS.overviewToday.group,
      PANEL_GROUPS.overviewToday.ids
    ),
    overviewHistory: loadPanelGroup(
      PANEL_GROUPS.overviewHistory.page,
      PANEL_GROUPS.overviewHistory.group,
      PANEL_GROUPS.overviewHistory.ids
    ),
    trendMain: loadPanelGroup(
      PANEL_GROUPS.trendMain.page,
      PANEL_GROUPS.trendMain.group,
      PANEL_GROUPS.trendMain.ids
    ),
  });

  /** 面板在 6 列网格中的默认跨列数。 */
  const PANEL_SPAN: Record<string, number> = {
    'today-cards': 6,
    'today-hourly': 6,
    'today-model-pie': 3,
    'today-agent-pie': 3,
    'today-model-table': 3,
    'today-agent-table': 3,
    'history-calls': 6,
    'history-tokens': 6,
    'trend-calls': 6,
    'trend-tokens': 6,
    'trend-heatmap': 4,
    'trend-weekday': 2,
    'trend-model-pie': 3,
    'trend-agent-pie': 3,
  };

  function persistGroup(gk: PanelGroupKey): void {
    const def = PANEL_GROUPS[gk];
    const g = panelState[gk];
    savePanelGroup(def.page, def.group, { order: g.order, hidden: g.hidden, sizes: g.sizes });
  }

  function togglePanel(gk: PanelGroupKey, id: string): void {
    const g = panelState[gk];
    g.hidden = g.hidden.includes(id) ? g.hidden.filter((x) => x !== id) : [...g.hidden, id];
    persistGroup(gk);
  }

  /** 面板生效跨列数：编辑模式把手拖出的档位优先，其次面板默认值。 */
  function spanOf(gk: PanelGroupKey, id: string): number {
    return panelState[gk].sizes[id]?.span ?? PANEL_SPAN[id] ?? 3;
  }

  /** 面板生效高度（px；未调整过 = 自适应）。 */
  function heightOf(gk: PanelGroupKey, id: string): number | undefined {
    return panelState[gk].sizes[id]?.height;
  }

  /** resize 把手拖动结束：记录 span/height 并持久化布局。 */
  function panelResize(gk: PanelGroupKey, id: string, span: number, height: number | undefined): void {
    const g = panelState[gk];
    g.sizes[id] = height === undefined ? { span } : { span, height };
    persistGroup(gk);
  }

  /** 重置布局（编辑模式）：清除当前页 localStorage 记录，恢复默认顺序/尺寸/显隐。 */
  function resetLayout(): void {
    if (tab === 'overview') {
      const def = PANEL_GROUPS.overviewToday;
      clearPanelPage(def.page);
      panelState.overviewToday = loadPanelGroup(def.page, def.group, def.ids);
      const defH = PANEL_GROUPS.overviewHistory;
      panelState.overviewHistory = loadPanelGroup(defH.page, defH.group, defH.ids);
    } else if (tab === 'trend') {
      const def = PANEL_GROUPS.trendMain;
      clearPanelPage(def.page);
      panelState.trendMain = loadPanelGroup(def.page, def.group, def.ids);
    }
  }

  /** 布局编辑模式：仅此时面板可拖拽/调整显隐（总览与趋势共用一个开关）。 */
  let editLayout = $state(false);

  let dragFrom = $state<{ gk: PanelGroupKey; index: number } | null>(null);
  let dropIndex = $state(-1);

  /**
   * 面板拖拽（Pointer Events；2026-09-26 修复“完全无法拖动”）：
   * WebView2 在 Tauri 默认 dragDropEnabled=true 时拦截 HTML5 drag 事件，
   * dragstart/drop 根本不触发。改为 pointerdown（Panel 内）+ window 级
   * pointermove/pointerup/pointercancel：move 时 elementsFromPoint 命中
   * 落点面板（data-panel-group/-index），抬起时交换 order 并持久化。
   */
  function panelPickStart(gk: PanelGroupKey, index: number, e: PointerEvent): void {
    e.preventDefault();
    dragFrom = { gk, index };
    dropIndex = index;
  }

  function panelPointerMove(e: PointerEvent): void {
    if (!dragFrom) return;
    for (const raw of document.elementsFromPoint(e.clientX, e.clientY)) {
      const card = (raw as Element).closest?.('.pcard');
      if (!card) continue;
      // 命中其它分组/页面区域：落点回退为自身（松手不产生交换）。
      if (card.getAttribute('data-panel-group') !== dragFrom.gk) {
        dropIndex = dragFrom.index;
        break;
      }
      const idx = Number(card.getAttribute('data-panel-index'));
      if (Number.isInteger(idx) && idx >= 0) dropIndex = idx;
      break;
    }
  }

  function panelPointerEnd(): void {
    const from = dragFrom;
    if (from && dropIndex >= 0 && dropIndex !== from.index) {
      const g = panelState[from.gk];
      const order = [...g.order];
      [order[from.index], order[dropIndex]] = [order[dropIndex], order[from.index]];
      g.order = order;
      persistGroup(from.gk);
    }
    dragFrom = null;
    dropIndex = -1;
  }

  $effect(() => {
    if (!dragFrom) return;
    window.addEventListener('pointermove', panelPointerMove);
    window.addEventListener('pointerup', panelPointerEnd);
    window.addEventListener('pointercancel', panelPointerEnd);
    return () => {
      window.removeEventListener('pointermove', panelPointerMove);
      window.removeEventListener('pointerup', panelPointerEnd);
      window.removeEventListener('pointercancel', panelPointerEnd);
    };
  });

  /** 本机日期（YYYY-MM-DD）：后端按统计时区日历解释，本机日期是最接近的代理。 */
  function localDay(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
      d.getDate()
    ).padStart(2, '0')}`;
  }

  const filters = $derived.by(() => ({
    agents: agentFilter ? [agentFilter] : [],
    providers: [] as string[],
    models: modelFilter ? [modelFilter] : [],
  }));

  const query = $derived.by((): SummaryQuery => {
    const now = new Date();
    if (rangeKey === 'today') {
      const day = localDay(now);
      return { first_day: day, last_day: day, granularity, ...filters };
    }
    if (rangeKey === '24h') {
      return {
        first_day: localDay(new Date(now.getTime() - 86_400_000)),
        last_day: localDay(now),
        granularity,
        ...filters,
      };
    }
    const days = Number(rangeKey);
    return {
      first_day: localDay(new Date(now.getTime() - (days - 1) * 86_400_000)),
      last_day: localDay(now),
      granularity,
      ...filters,
    };
  });

  /** 今日分区查询：当日 first=last（granularity 不影响今日小时数据）。 */
  const todayQuery = $derived.by((): SummaryQuery => {
    const day = localDay(new Date());
    return { first_day: day, last_day: day, granularity: 'day', ...filters };
  });

  const agentsAvailable = $derived(
    Array.from(new Set((summary?.agents ?? []).map((a) => a.agent))).sort()
  );
  const modelsAvailable = $derived(
    Array.from(new Set((summary?.models ?? []).map((m) => m.model ?? '__unknown__'))).sort()
  );

  // 下拉选项 = 用户列表 +（当前用户不在列表时合成一项，如初始 default）。
  const userOptions = $derived.by(() => {
    const list = [...users];
    if (currentUser && !list.some((u) => u.user_id === currentUser)) {
      list.unshift({ user_id: currentUser, name: currentUser, created_at_ms: 0 });
    }
    return list;
  });

  // 今日饼图数据：total_tokens 占比（unknown 归“未知”；0 值不参与）。
  const todayModelPie = $derived(
    (todaySummary?.models ?? [])
      .map((m) => ({
        name: m.model ?? t('common.unknown'),
        value: Number(m.sums.total_tokens_known ?? 0),
      }))
      .filter((d) => d.value > 0)
  );
  const todayAgentPie = $derived(
    (todaySummary?.agents ?? [])
      .map((a) => ({ name: a.agent, value: Number(a.sums.total_tokens_known ?? 0) }))
      .filter((d) => d.value > 0)
  );

  // 趋势页饼图数据：当前时间范围 summary 的模型/Agent total_tokens 占比。
  const trendModelPie = $derived(
    (summary?.models ?? [])
      .map((m) => ({
        name: m.model ?? t('common.unknown'),
        value: Number(m.sums.total_tokens_known ?? 0),
      }))
      .filter((d) => d.value > 0)
  );
  const trendAgentPie = $derived(
    (summary?.agents ?? [])
      .map((a) => ({ name: a.agent, value: Number(a.sums.total_tokens_known ?? 0) }))
      .filter((d) => d.value > 0)
  );

  // 趋势页范围汇总（2026-09-26）：totals 汇总 + 会话数按周期求和
  // （distinct_sessions 全部未知时显示 —，不补零）。
  const trendSessions = $derived.by(() => {
    let sum = 0;
    let known = false;
    for (const p of summary?.periods ?? []) {
      if (p.distinct_sessions !== null) {
        sum += p.distinct_sessions;
        known = true;
      }
    }
    return known ? sum : null;
  });

  const trendSummaryCards = $derived.by(() => {
    const totals = summary?.totals;
    if (!totals) return [];
    const avgMs = totals.avg_duration_ms === null ? null : Number(totals.avg_duration_ms);
    return [
      { key: 'calls', label: t('trend.calls'), value: fmtSmart(totals.call_count) },
      {
        key: 'input',
        label: t('cards.input'),
        value: fmtSmart(totals.input_total_known),
        hint: t('cards.input.hint'),
        sub: t('overview.breakdown.input', {
          hit: fmtSmart(totals.cache_read_known),
          miss: fmtSmart(totals.uncached_known),
        }),
      },
      { key: 'output', label: t('cards.output'), value: fmtSmart(totals.output_total_known) },
      { key: 'total', label: t('cards.total'), value: fmtSmart(totals.total_tokens_known) },
      { key: 'ratio', label: t('cards.cacheRatio'), value: fmtPercent(totals.cache_input_ratio) },
      { key: 'sessions', label: t('trend.sessions'), value: fmtSmart(trendSessions) },
      { key: 'avg', label: t('cards.avgDuration'), value: fmtDurationShort(avgMs) },
    ];
  });

  /** 今日分区日期标签。 */
  const todayDateLabel = $derived(new Date().toLocaleDateString(i18n.locale));

  function panelTitle(id: string): string {
    switch (id) {
      case 'today-cards':
        return t('overview.today');
      case 'today-hourly':
        return t('hourly.title');
      case 'today-model-pie':
        return t('overview.todayPie.model');
      case 'today-agent-pie':
        return t('overview.todayPie.agent');
      case 'today-model-table':
        return t('overview.todayTable.model');
      case 'today-agent-table':
        return t('overview.todayTable.agent');
      case 'history-calls':
      case 'trend-calls':
        return t('trend.chart.calls');
      case 'history-tokens':
      case 'trend-tokens':
        return t('trend.chart.tokens');
      case 'trend-heatmap':
        return t('heatmap.title');
      case 'trend-weekday':
        return t('trend.weekday');
      case 'trend-model-pie':
        return t('trend.pie.model');
      case 'trend-agent-pie':
        return t('trend.pie.agent');
      default:
        return id;
    }
  }

  async function loadSettings() {
    try {
      settings = await api.getSettings();
      initLocale(settings.language);
    } catch (e) {
      loadError = parseError(e);
    }
  }

  /** 主查询 + 今日查询（同一次用户交互/刷新内成对加载）。 */
  async function loadSummary() {
    queryError = '';
    const [q, tq] = [query, todayQuery];
    try {
      const [s, ts] = await Promise.all([api.summary(q), api.summary(tq)]);
      summary = s;
      todaySummary = ts;
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

  async function loadUsers() {
    try {
      const r = await api.listUsers();
      users = r.users;
      currentUser = r.current;
      userSelect = r.current;
    } catch (e) {
      userError = parseError(e);
    }
  }

  /** 用户切换/导入等不改变 query 的数据重载（summary/heatmap/sources 都要刷新）。 */
  async function reloadUserData() {
    dataReloadKey += 1;
    await Promise.all([loadSummary(), loadSources()]);
  }

  async function onUserChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value;
    if (value === '__new__') {
      // 打开新建行；下拉回落到当前用户。
      newUserOpen = true;
      newUserName = '';
      userSelect = currentUser;
      return;
    }
    if (!value || value === currentUser) {
      userSelect = currentUser;
      return;
    }
    userBusy = true;
    userError = '';
    try {
      await api.setCurrentUser(value);
      currentUser = value;
      userSelect = value;
      // summary/heatmap 后端按当前用户过滤，切换后必须重拉。
      await reloadUserData();
    } catch (e) {
      userError = parseError(e);
      userSelect = currentUser;
    } finally {
      userBusy = false;
    }
  }

  function cancelCreateUser() {
    newUserOpen = false;
    newUserName = '';
  }

  async function confirmCreateUser() {
    const name = newUserName.trim();
    if (!name || userBusy) return;
    userBusy = true;
    userError = '';
    try {
      await api.createUser(name, true);
      newUserOpen = false;
      newUserName = '';
      await loadUsers();
      await reloadUserData();
    } catch (e) {
      userError = parseError(e);
    } finally {
      userBusy = false;
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
      await Promise.all([loadSummary(), loadSources(), loadUsers(), pollRefresh()]);
    })();
    const pollTimer = setInterval(() => void pollRefresh(), 3000);
    return () => clearInterval(pollTimer);
  });

  // 近 24 小时/当天自动切到小时粒度（任务 C7）。
  $effect(() => {
    if (rangeKey === '24h' || rangeKey === 'today') granularity = 'hour';
  });

  // 查询条件变化即重查（防抖 300ms）。
  let queryTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void query;
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

  // 采集中：ETA 文案（null/不可估时为空串，模板据此隐藏）。
  const refreshEtaText = $derived(fmtEtaDuration(refresh?.eta_seconds));
</script>

<main>
  <header>
    <h1><img src="/brand/app-icon.svg" width="28" height="28" alt="" />{t('app.title')}</h1>
    <span class="subtitle">{t('app.subtitle')}</span>
    <span class="spacer"></span>
    {#if summary}
      <span class="muted">{t('cards.revision', { revision: summary.data_revision })}</span>
    {/if}
    {#if newUserOpen}
      <span class="user-create">
        <input
          placeholder={t('users.namePlaceholder')}
          bind:value={newUserName}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void confirmCreateUser();
            }
          }}
        />
        <button class="primary" disabled={userBusy || newUserName.trim() === ''} onclick={() => void confirmCreateUser()}>
          {t('users.createConfirm')}
        </button>
        <button disabled={userBusy} onclick={cancelCreateUser}>{t('users.createCancel')}</button>
      </span>
    {:else}
      <label class="user-picker">
        {t('users.label')}
        <select value={userSelect} disabled={userBusy} onchange={onUserChange}>
          {#each userOptions as u (u.user_id)}
            <option value={u.user_id}>{u.name}</option>
          {/each}
          <option value="__new__">{t('users.create')}</option>
        </select>
      </label>
    {/if}
    {#if refresh?.running}
      <span class="refresh-progress" role="status" aria-live="polite">
        <span class="rbar" aria-hidden="true">
          <span class="rfill" style:width="{refresh.progress_percent}%"></span>
        </span>
        <span class="rlabel">
          {t('refresh.progress', { percent: refresh.progress_percent })}
          {#if refreshEtaText}
            <span class="reta">· {t('refresh.eta', { eta: refreshEtaText })}</span>
          {/if}
        </span>
      </span>
    {/if}
    <button class="primary" disabled={refreshing || refresh?.running} onclick={manualRefresh}>
      {refreshLabel}
    </button>
  </header>

  <nav>
    {#each [['overview', t('nav.overview')], ['trend', t('nav.trend')], ['sources', t('nav.sources')], ['details', t('nav.details')], ['settings', t('nav.settings')]] as [id, label] (id)}
      <button class:active={tab === id} onclick={() => (tab = id as Tab)}>{label}</button>
    {/each}
  </nav>

  {#if loadError}
    <p class="error">{t('common.error', { message: loadError })}</p>
  {/if}
  {#if userError}
    <p class="error">{t('users.actionFailed', { message: userError })}</p>
  {/if}

  {#if tab === 'overview' || tab === 'trend' || tab === 'details'}
    <div class="filters">
      {#if tab === 'trend'}
        <label>{t('filter.range')}
          <select bind:value={rangeKey}>
            <option value="24h">{t('filter.quick.24h')}</option>
            <option value="today">{t('filter.quick.today')}</option>
            <option value="7">{t('filter.quick.7')}</option>
            <option value="30">{t('filter.quick.30')}</option>
            <option value="365">{t('filter.quick.365')}</option>
          </select>
        </label>
        <label>{t('filter.granularity.label')}
          <select bind:value={granularity}>
            <option value="hour">{t('filter.granularity.hour')}</option>
            <option value="day">{t('filter.granularity.day')}</option>
            <option value="week">{t('filter.granularity.week')}</option>
            <option value="month">{t('filter.granularity.month')}</option>
          </select>
        </label>
      {/if}
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
      {#if tab === 'overview' || tab === 'trend'}
        <span class="spacer"></span>
        {#if editLayout}
          <button type="button" class="edit-toggle" onclick={resetLayout}>
            ↺ {t('panel.reset')}
          </button>
        {/if}
        <button
          type="button"
          class="edit-toggle"
          class:active={editLayout}
          onclick={() => (editLayout = !editLayout)}
        >
          🔧 {editLayout ? t('panel.editDone') : t('panel.edit')}
        </button>
      {/if}
    </div>
    {#if editLayout && (tab === 'overview' || tab === 'trend')}
      <p class="edit-hint">{t('panel.editHint')}</p>
    {/if}
  {/if}

  {#if queryError}
    <p class="error">{t('common.error', { message: queryError })}</p>
  {/if}

  {#if tab === 'overview'}
    {#if summary}
      {#if summary.totals.call_count === 0 && summary.periods.length === 0}
        <p class="empty">{t('common.empty')}</p>
      {:else}
        <div class="section-head">
          <h2>{t('overview.todaySection')}</h2>
          <span class="section-date">{todayDateLabel}</span>
        </div>
        <div class="panel-grid">
          {#each panelState.overviewToday.order as id, i (id)}
            <Panel
              title={panelTitle(id)}
              span={spanOf('overviewToday', id)}
              height={heightOf('overviewToday', id)}
              hidden={panelState.overviewToday.hidden.includes(id)}
              editable={editLayout}
              dragging={dragFrom?.gk === 'overviewToday' && dragFrom.index === i}
              dropTarget={dragFrom?.gk === 'overviewToday' && dropIndex === i}
              panelGroup="overviewToday"
              panelIndex={i}
              ontoggle={() => togglePanel('overviewToday', id)}
              onpickstart={(e) => panelPickStart('overviewToday', i, e)}
              onsize={(span, height) => panelResize('overviewToday', id, span, height)}
            >
              {#if id === 'today-cards'}
                {#if todaySummary}
                  <TodayOverview totals={todaySummary.totals} hourly={todaySummary.today_hourly} />
                {:else}
                  <p class="muted">{t('common.loading')}</p>
                {/if}
              {:else if id === 'today-hourly'}
                <TodayHourly hourly={todaySummary?.today_hourly ?? []} query={todayQuery} {isDark} />
              {:else if id === 'today-model-pie'}
                <SharePie data={todayModelPie} {isDark} />
              {:else if id === 'today-agent-pie'}
                <SharePie data={todayAgentPie} {isDark} />
              {:else if id === 'today-model-table'}
                <BreakdownTables models={todaySummary?.models ?? []} kind="model" />
              {:else if id === 'today-agent-table'}
                <BreakdownTables agents={todaySummary?.agents ?? []} kind="agent" />
              {/if}
            </Panel>
          {/each}
        </div>

        <!-- 范围/粒度只影响历史趋势区（今日数据独立查询），故放在该分区内侧而非页面顶部。 -->
        <div class="section-head">
          <h2>{t('overview.historySection')}</h2>
          <span class="section-controls" title={t('overview.historyFilterHint')}>
            <label>{t('filter.range')}
              <select bind:value={rangeKey}>
                <option value="24h">{t('filter.quick.24h')}</option>
                <option value="today">{t('filter.quick.today')}</option>
                <option value="7">{t('filter.quick.7')}</option>
                <option value="30">{t('filter.quick.30')}</option>
                <option value="365">{t('filter.quick.365')}</option>
              </select>
            </label>
            <label>{t('filter.granularity.label')}
              <select bind:value={granularity}>
                <option value="hour">{t('filter.granularity.hour')}</option>
                <option value="day">{t('filter.granularity.day')}</option>
                <option value="week">{t('filter.granularity.week')}</option>
                <option value="month">{t('filter.granularity.month')}</option>
              </select>
            </label>
          </span>
        </div>
        <div class="panel-grid">
          {#each panelState.overviewHistory.order as id, i (id)}
            <Panel
              title={panelTitle(id)}
              span={spanOf('overviewHistory', id)}
              height={heightOf('overviewHistory', id)}
              hidden={panelState.overviewHistory.hidden.includes(id)}
              editable={editLayout}
              dragging={dragFrom?.gk === 'overviewHistory' && dragFrom.index === i}
              dropTarget={dragFrom?.gk === 'overviewHistory' && dropIndex === i}
              panelGroup="overviewHistory"
              panelIndex={i}
              ontoggle={() => togglePanel('overviewHistory', id)}
              onpickstart={(e) => panelPickStart('overviewHistory', i, e)}
              onsize={(span, height) => panelResize('overviewHistory', id, span, height)}
            >
              {#if id === 'history-calls'}
                <CallsChart periods={summary.periods} {query} {granularity} {isDark} />
              {:else if id === 'history-tokens'}
                <TokenChart periods={summary.periods} {query} {granularity} {isDark} />
              {/if}
            </Panel>
          {/each}
        </div>
      {/if}
    {:else}
      <p class="muted">{t('common.loading')}</p>
    {/if}
  {:else if tab === 'trend'}
    {#if summary}
      <!-- 范围汇总面板（图表区上方固定位置）：7 张小卡片，取 summary.totals。 -->
      <div class="range-summary">
        <div class="section-head">
          <h2>{t('trend.summary')}</h2>
        </div>
        <div class="summary-cards">
          {#each trendSummaryCards as c (c.key)}
            <div class="scard" title={c.hint ?? ''}>
              <div class="slabel">
                {c.label}{#if c.hint}<span class="shint">{c.hint}</span>{/if}
              </div>
              <div class="svalue">{c.value}</div>
              {#if c.sub}<div class="ssub">{c.sub}</div>{/if}
            </div>
          {/each}
        </div>
      </div>
      <div class="panel-grid">
        {#each panelState.trendMain.order as id, i (id)}
          <Panel
            title={panelTitle(id)}
            span={spanOf('trendMain', id)}
            height={heightOf('trendMain', id)}
            hidden={panelState.trendMain.hidden.includes(id)}
            editable={editLayout}
            dragging={dragFrom?.gk === 'trendMain' && dragFrom.index === i}
            dropTarget={dragFrom?.gk === 'trendMain' && dropIndex === i}
            panelGroup="trendMain"
            panelIndex={i}
            ontoggle={() => togglePanel('trendMain', id)}
            onpickstart={(e) => panelPickStart('trendMain', i, e)}
            onsize={(span, height) => panelResize('trendMain', id, span, height)}
          >
            {#if id === 'trend-calls'}
              <CallsChart periods={summary.periods} {query} {granularity} {isDark} />
            {:else if id === 'trend-tokens'}
              <TokenChart periods={summary.periods} {query} {granularity} {isDark} />
            {:else if id === 'trend-heatmap'}
              <UsageHeatmap {query} reloadKey={dataReloadKey} oncells={(cells) => (heatmapCells = cells)} {isDark} />
            {:else if id === 'trend-weekday'}
              <WeekdayBar cells={heatmapCells} {isDark} />
            {:else if id === 'trend-model-pie'}
              <SharePie data={trendModelPie} {isDark} />
            {:else if id === 'trend-agent-pie'}
              <SharePie data={trendAgentPie} {isDark} />
            {/if}
          </Panel>
        {/each}
      </div>
    {:else}
      <p class="muted">{t('common.loading')}</p>
    {/if}
  {:else if tab === 'details'}
    <EventDetails {query} reloadKey={dataReloadKey} />
  {:else if tab === 'sources'}
    <SourceList {sources} users={userOptions} onchanged={loadSources} />
  {:else if tab === 'settings'}
    {#if settings}
      <SettingsPanel settings={settings} collecting={!!refresh?.running} onsaved={onSettingsSaved} ondatachanged={() => void reloadUserData()} />
    {/if}
  {/if}
</main>

<style>
  main {
    padding: 12px 20px 32px;
    font-family: system-ui, 'Segoe UI', sans-serif;
    color: var(--text);
    background: var(--bg);
    min-height: 100vh;
    box-sizing: border-box;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  h1 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    margin: 0;
  }
  .subtitle {
    color: var(--text-muted);
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
  nav {
    display: flex;
    gap: 4px;
    padding: 6px 0;
  }
  nav button {
    border: none;
    background: transparent;
    padding: 4px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }
  nav button.active {
    background: var(--bg-nav-active);
    color: var(--accent);
    font-weight: 600;
  }
  .filters {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 4px 0;
    font-size: 12.5px;
    flex-wrap: wrap;
  }
  .filters label {
    display: flex;
    gap: 4px;
    align-items: center;
    color: var(--text-secondary);
  }
  .filters .spacer {
    flex: 1;
  }
  /* 布局编辑模式开关（🔧；激活时高亮）。 */
  .edit-toggle {
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: 6px;
    padding: 3px 12px;
    cursor: pointer;
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .edit-toggle.active {
    background: var(--accent-bg);
    color: var(--accent);
    border-color: var(--accent);
    font-weight: 600;
  }
  .edit-hint {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--warning);
  }
  select {
    padding: 2px 4px;
    font-size: 12.5px;
  }
  .user-picker {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .user-picker select {
    max-width: 170px;
  }
  /* 采集中：简洁进度条（宽度 = 百分比）+ 百分比/剩余时间文案。 */
  .refresh-progress {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .refresh-progress .rbar {
    display: inline-block;
    width: 110px;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-skeleton);
    overflow: hidden;
  }
  .refresh-progress .rfill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transition: width 0.3s ease;
  }
  .refresh-progress .rlabel {
    font-variant-numeric: tabular-nums;
  }
  .refresh-progress .reta {
    color: var(--text-muted);
  }
  .user-create {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .user-create input {
    padding: 4px 8px;
    font-size: 12.5px;
    width: 150px;
    box-sizing: border-box;
  }
  .user-create button {
    padding: 4px 12px;
    cursor: pointer;
    font-size: 12.5px;
  }
  button.primary {
    background: var(--accent);
    color: var(--accent-text);
    border: none;
    border-radius: 6px;
    padding: 5px 14px;
    cursor: pointer;
    font-size: 12.5px;
  }
  button.primary:disabled {
    background: var(--accent);
    opacity: 0.55;
    cursor: wait;
  }
  .error {
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 13px;
  }
  .empty {
    color: var(--text-muted);
    background: var(--bg-hover);
    border-radius: 8px;
    padding: 32px;
    text-align: center;
  }
  .muted {
    color: var(--text-muted);
    font-size: 13px;
  }
  /* 今日/历史醒目分区标题（任务 D8）；历史区标题右侧内联范围/粒度筛选（只影响该区）。 */
  .section-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 14px 0 6px;
    flex-wrap: wrap;
  }
  .section-head h2 {
    font-size: 15px;
    margin: 0;
    padding-left: 10px;
    border-left: 4px solid var(--accent);
    color: var(--text-heading);
  }
  .section-date {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  /* 历史趋势区标题右侧的范围/粒度下拉（与今日数据区视觉分离）。 */
  .section-controls {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .section-controls label {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  /* 面板 6 列格子布局（任务 E10）；跨列数由 Panel 的 span 类决定。 */
  .panel-grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 12px;
    align-items: stretch;
  }
  /* 趋势页范围汇总（图表区上方）：一行水平卡片组（7 张小卡片）。 */
  .range-summary {
    margin-bottom: 4px;
  }
  .range-summary .section-head {
    margin: 4px 0 6px;
  }
  .summary-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 8px;
    padding: 8px 0 10px;
  }
  .scard {
    background: var(--bg-code);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 12px;
    min-width: 0;
  }
  .scard .slabel {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .scard .slabel .shint {
    color: var(--text-muted);
    font-size: 11px;
  }
  .scard .svalue {
    font-size: 18px;
    font-weight: 600;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  .scard .ssub {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
  @media (max-width: 900px) {
    .panel-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
