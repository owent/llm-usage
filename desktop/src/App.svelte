<script lang="ts">
  import { onMount } from 'svelte';
  import BudgetReminder from './components/BudgetReminder.svelte';
  import { calendarDay, offsetDay } from './lib/calendar';
  import { api, parseError } from './lib/api';
  import type {
    AppSettings,
    CostSummaryDto,
    RefreshStateDto,
    SourceDto,
    SummaryDto,
    SummaryQuery,
    UserDto,
  } from './lib/api';
  import { i18n, initLocale, normalizeLocale, setLocale, t, fmtEtaDuration, fmtSmart, fmtPercent, fmtDurationShort } from './lib/i18n.svelte';
  import { loadPanelGroup, savePanelGroup, clearPanelPage } from './lib/panels';
  import Icon from './components/Icon.svelte';
  import UsageInsights from './components/UsageInsights.svelte';
  import Panel from './components/Panel.svelte';
  import CallsChart from './components/CallsChart.svelte';
  import TokenChart from './components/TokenChart.svelte';
  import TodayHourly from './components/TodayHourly.svelte';
  import UsageHeatmap from './components/UsageHeatmap.svelte';
  import CostPanel from './components/CostPanel.svelte';
  import CostReferenceSummary from './components/CostReferenceSummary.svelte';
  import CostChart from './components/CostChart.svelte';
  import TodayOverview from './components/TodayOverview.svelte';
  import QuotaCard from './components/QuotaCard.svelte';
  import TelemetrySetup from './components/TelemetrySetup.svelte';
  import { checkTelemetry } from './lib/telemetry.svelte';
  import BreakdownTables from './components/BreakdownTables.svelte';
  import SourceList from './components/SourceList.svelte';
  import SettingsPanel from './components/SettingsPanel.svelte';
  import SharePie from './components/SharePie.svelte';
  import WeekdayBar from './components/WeekdayBar.svelte';
  import EventDetails from './components/EventDetails.svelte';

  type Tab = 'overview' | 'trend' | 'sources' | 'details' | 'settings';
  type Granularity = 'hour' | 'day' | 'week' | 'month';
  /** Today/two-calendar-day ranges use hourly data bounded by dates in the configured timezone. */
  type RangeKey = '2' | 'today' | '7' | '30' | '365';

  let tab = $state<Tab>('overview');
  let settingsSection = $state<'general' | 'telemetry'>('general');
  let settings = $state<AppSettings | null>(null);
  let ready = $state(false);
  let clockNow = $state(new Date());
  const todayDay = $derived(calendarDay(clockNow, settings?.timezone ?? 'UTC'));
  let catalog = $state<SummaryDto | null>(null);
  let summarySequence = 0;
  let pendingSummaryKey = '';
  let summaryLoading = $state(false);
  let summary = $state<SummaryDto | null>(null);
  /** Separate today query (D8): summary first_day=last_day for the current-day section. */
  let todaySummary = $state<SummaryDto | null>(null);
  let sources = $state<SourceDto[]>([]);
  let refresh = $state<RefreshStateDto | null>(null);
  let loadError = $state('');
  let queryError = $state('');
  let granularity = $state<Granularity>('day');
  let rangeKey = $state<RangeKey>('30');
  let agentFilter = $state('');
  let modelFilter = $state('');

  // Theme: system/light/dark through data-theme and derived isDark for ECharts.
  /** Track system dark-mode changes so system-theme charts update immediately. */
  let systemDark = $state(window.matchMedia('(prefers-color-scheme: dark)').matches);

  $effect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = (e: MediaQueryListEvent) => (systemDark = e.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  });

  /** Explicit light/dark wins; other values, including unknown settings, use system. */
  const theme = $derived(
    settings?.theme === 'light' || settings?.theme === 'dark' ? settings.theme : 'system'
  );
  /** Effective dark mode supplied to ECharts: explicit dark or system with a dark preference. */
  const isDark = $derived(theme === 'dark' || (theme === 'system' && systemDark));

  // For system theme, remove data-theme and let themes.css prefers-color-scheme apply.
  $effect(() => { document.documentElement.lang = i18n.locale; });
  $effect(() => {
    if (theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
  });

  // v6 users: header selection and creation.
  let users = $state<UserDto[]>([]);
  let currentUser = $state('');
  let userSelect = $state('');
  let newUserOpen = $state(false);
  let newUserName = $state('');
  let userBusy = $state(false);
  let userError = $state('');

  /** Force child queries to reload after user switches/imports without a changed query. */
  let dataReloadKey = $state(0);

  // E10/E11 panel order/visibility, persisted per page in localStorage.
  type PanelGroupKey = 'overviewToday' | 'overviewHistory' | 'trendMain';
  const PANEL_GROUPS: Record<PanelGroupKey, { page: string; group: string; ids: string[] }> = {
    overviewToday: {
      page: 'overview',
      group: 'today',
      ids: [
        'today-cards',
        'today-hourly',
        'today-costs',
        'today-model-pie',
        'today-agent-pie',
        'today-model-table',
        'today-agent-table',
      ],
    },
    overviewHistory: {
      page: 'overview',
      group: 'history',
      ids: ['history-tokens', 'history-calls'],
    },
    trendMain: {
      page: 'trend',
      group: 'main',
      ids: [
        'trend-tokens',
        'trend-calls',
        'trend-costs',
        'trend-model-pie',
        'trend-agent-pie',
        'trend-model-table',
        'trend-heatmap',
        'trend-weekday',
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

  /** Default panel column span in the six-column grid. */
  const PANEL_SPAN: Record<string, number> = {
    'today-cards': 6,
    'today-hourly': 6,
    'today-costs': 6,
    'today-model-pie': 3,
    'today-agent-pie': 3,
    'today-model-table': 6,
    'today-agent-table': 6,
    'history-calls': 6,
    'history-tokens': 6,
    'trend-calls': 6,
    'trend-tokens': 6,
    'trend-costs': 6,
    'trend-heatmap': 6,
    'trend-weekday': 6,
    'trend-model-pie': 3,
    'trend-agent-pie': 3,
    'trend-model-table': 6,
  };

  function persistGroup(gk: PanelGroupKey): void {
    const def = PANEL_GROUPS[gk];
    const g = panelState[gk];
    savePanelGroup(def.page, def.group, { order: g.order, hidden: g.hidden, sizes: g.sizes });
  }

  const todayPanelOrder = $derived(panelState.overviewToday.order.filter(
    (id) => id !== 'today-costs' || (settings?.pricing?.enabled ?? false),
  ));

  /** Hide trend cost panels when estimates are disabled, without rewriting the saved layout. */
  const trendPanelOrder = $derived(
    panelState.trendMain.order.filter(
      (id) => id !== 'trend-costs' || (settings?.pricing?.enabled ?? false),
    ).filter((id) => id !== 'trend-heatmap' && id !== 'trend-weekday')
      .concat(panelState.trendMain.order.filter((id) => id === 'trend-heatmap' || id === 'trend-weekday')),
  );

  function togglePanel(gk: PanelGroupKey, id: string): void {
    const g = panelState[gk];
    g.hidden = g.hidden.includes(id) ? g.hidden.filter((x) => x !== id) : [...g.hidden, id];
    persistGroup(gk);
  }

  /** Prefer a resized edit-mode column span over the panel default. */
  function spanOf(gk: PanelGroupKey, id: string): number {
    if (id.endsWith('model-table')) return 6;
    return panelState[gk].sizes[id]?.span ?? PANEL_SPAN[id] ?? 3;
  }

  /** Panel height in pixels; unmodified panels size automatically. */
  function heightOf(gk: PanelGroupKey, id: string): number | undefined {
    return panelState[gk].sizes[id]?.height;
  }

  /** Persist span/height and layout when resize dragging ends. */
  function panelResize(gk: PanelGroupKey, id: string, span: number, height: number | undefined): void {
    const g = panelState[gk];
    g.sizes[id] = height === undefined ? { span } : { span, height };
    persistGroup(gk);
  }

  /** Reset current-page localStorage layout to default order, dimensions and visibility. */
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

  /** Shared overview/trend edit mode enables dragging and visibility controls. */
  let editLayout = $state(false);

  let dragFrom = $state<{ gk: PanelGroupKey; index: number } | null>(null);
  let dropIndex = $state(-1);

  /**
   * Pointer-based panel dragging fixed on 2026-09-26. With default dragDropEnabled=true,
   * Tauri WebView2 intercepted HTML5 dragstart/drop. Use panel pointerdown and window
   * pointermove/pointerup/pointercancel instead. elementsFromPoint identifies the target
   * data-panel-group/-index during movement; releasing swaps and persists panel order.
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
      // A target in another group/page falls back to this panel, leaving its order unchanged.
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
      const [moved] = order.splice(from.index, 1);
      order.splice(dropIndex, 0, moved);
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
    const cancel = () => { dragFrom = null; dropIndex = -1; };
    window.addEventListener('pointercancel', cancel);
    return () => {
      window.removeEventListener('pointermove', panelPointerMove);
      window.removeEventListener('pointerup', panelPointerEnd);
      window.removeEventListener('pointercancel', cancel);
    };
  });

  const filters = $derived.by(() => ({
    agents: agentFilter ? [agentFilter] : [],
    providers: [] as string[],
    models: modelFilter ? [modelFilter] : [],
  }));

  const query = $derived.by((): SummaryQuery => ({
    first_day: offsetDay(todayDay, -(rangeKey === 'today' ? 0 : Number(rangeKey) - 1)),
    last_day: todayDay, granularity, ...filters,
  }));

  const todayQuery = $derived.by((): SummaryQuery => ({
    first_day: todayDay, last_day: todayDay, granularity: 'day', ...filters,
  }));

  let todaySelection = $state<{first:string;last:string}|null>(null);
  let todaySelectionSummary = $state<SummaryDto|null>(null);
  let todaySelectionError = $state('');
  let todaySelectionCosts = $state<CostSummaryDto|null>(null);
  let todaySelectionCostsError = $state('');
  const todayScopeSummary = $derived(todaySelection ? todaySelectionSummary : todaySummary);
  const todaySelectionQuery = $derived.by((): SummaryQuery|null => todaySelection ? {
    ...todayQuery, granularity:'hour',first_period:todaySelection.first,last_period:todaySelection.last,
  } : null);
  function onTodayRangeChange(first:string,last:string): void {
    const start=first===todayDay ? `${todayDay} 00:00` : first;
    const end=last===todayDay ? `${todayDay} 23:00` : last;
    if(!start.startsWith(todayDay+' ') || !end.startsWith(todayDay+' ')) return;
    if(todaySelection?.first!==start || todaySelection.last!==end) todaySelection={first:start,last:end};
  }
  $effect(()=>{
    void todayQuery;void currentUser;void settings?.timezone;void settings?.week_start;
    todaySelection=null;
  });
  $effect(()=>{
    const q=todaySelectionQuery;
    void todaySummary?.data_revision;void dataReloadKey;void currentUser;void settings?.timezone;void settings?.week_start;
    todaySelectionSummary=null;todaySelectionError='';
    if(!q || tab!=='overview' || !ready) return;
    let cancelled=false;
    api.summary(q).then((result)=>{if(!cancelled)todaySelectionSummary=result;})
      .catch((error)=>{if(!cancelled)todaySelectionError=parseError(error);});
    return ()=>{cancelled=true;};
  });
  $effect(()=>{
    const q=todaySelectionQuery;
    void todaySummary?.data_revision;void dataReloadKey;void currentUser;void settings?.timezone;void settings?.week_start;void settings?.pricing;
    todaySelectionCosts=null;todaySelectionCostsError='';
    if(!q || tab!=='overview' || !ready || !settings?.pricing?.enabled) return;
    let cancelled=false;
    api.costSummary(q).then((result)=>{if(!cancelled)todaySelectionCosts=result;})
      .catch((error)=>{if(!cancelled)todaySelectionCostsError=parseError(error);});
    return ()=>{cancelled=true;};
  });

  // One cost query feeds the summary, model rows and curve on the active page.
  let costs = $state<CostSummaryDto | null>(null);
  let costsError = $state('');
  const todayScopeCosts = $derived(todaySelection ? todaySelectionCosts : costs);
  const todayScopeCostsError = $derived(todaySelection ? todaySelectionCostsError : costsError);
  $effect(() => {
    const q = tab === 'overview' ? todayQuery : query;
    const pricing = settings?.pricing;
    void currentUser; void dataReloadKey; void summary?.data_revision;
    void todaySummary?.data_revision; void settings?.timezone;
    costs = null; costsError = '';
    if (!ready || !pricing?.enabled || (tab !== 'overview' && tab !== 'trend')) return;
    let cancelled = false;
    api.costSummary(q).then((result) => { if (!cancelled) costs = result; })
      .catch((e) => { if (!cancelled) costsError = parseError(e); });
    return () => { cancelled = true; };
  });

  let trendSelection = $state<{first: string; last: string} | null>(null);
  let selectionSummary = $state<SummaryDto | null>(null);
  let selectionError = $state('');
  let selectionCosts = $state<CostSummaryDto|null>(null);
  let selectionCostsError = $state('');
  const trendScopeCosts = $derived(trendSelection ? selectionCosts : costs);
  const trendScopeSummary = $derived(trendSelection ? selectionSummary : summary);
  const trendRangeCaption = $derived.by(() => {
    if (!trendSelection) return t('dashboard.queryRange', {range: `${query.first_day} ~ ${query.last_day}`});
    const {first,last} = trendSelection;
    let range = first === last ? first : `${first} ~ ${last}`;
    const periods = summary?.periods.filter((p) => p.label >= first && p.label <= last) ?? [];
    if (periods.length && (granularity === 'week' || granularity === 'month')) {
      const start = periods[0].start_day > query.first_day ? periods[0].start_day : query.first_day;
      const end = periods[periods.length-1].end_day < query.last_day ? periods[periods.length-1].end_day : query.last_day;
      range += ` · ${start} ~ ${end}`;
    }
    return t('dashboard.selectedRange', {range});
  });
  function onTrendRangeChange(first: string, last: string): void {
    // Cost curves are daily when usage is hourly; include all visible hours of the day.
    const labels = summary?.periods.map((p) => p.label) ?? [];
    const firstLabel = labels.find((label) => label === first || label.startsWith(first + ' '));
    const lastLabel = [...labels].reverse().find((label) => label === last || label.startsWith(last + ' '));
    if (!firstLabel || !lastLabel) return;
    if (trendSelection?.first === firstLabel && trendSelection?.last === lastLabel) return;
    trendSelection = {first: firstLabel, last: lastLabel};
  }
  function onTrendPeriodClick(label: string): void { onTrendRangeChange(label, label); }
  $effect(() => {
    const selected = trendSelection;
    const q = query;
    void summary?.data_revision; void dataReloadKey; void currentUser;
    void settings?.timezone; void settings?.week_start;
    selectionSummary = null; selectionError = '';
    if (!selected || (tab !== 'trend' && tab !== 'overview')) return;
    const periods = summary?.periods.filter((p) => p.label >= selected.first && p.label <= selected.last) ?? [];
    if (!periods.length) return;
    const scoped = {...q,
      first_day: periods[0].start_day > q.first_day ? periods[0].start_day : q.first_day,
      last_day: periods[periods.length - 1].end_day < q.last_day ? periods[periods.length - 1].end_day : q.last_day,
      first_period: selected.first, last_period: selected.last};
    let cancelled = false;
    api.summary(scoped).then((result) => {if (!cancelled) selectionSummary = result;})
      .catch((e) => {if (!cancelled) selectionError = parseError(e);});
    return () => {cancelled = true;};
  });

  $effect(() => {
    const selected=trendSelection; const q=query;
    void settings?.timezone; void settings?.week_start; void settings?.pricing;
    void summary?.data_revision; void dataReloadKey; void currentUser;
    selectionCosts=null; selectionCostsError='';
    if(!selected || (tab!=='trend' && tab!=='overview') || !settings?.pricing?.enabled) return;
    const periods=summary?.periods.filter((p)=>p.label>=selected.first && p.label<=selected.last)??[];
    if(!periods.length) return;
    const scoped={...q,first_day:periods[0].start_day>q.first_day?periods[0].start_day:q.first_day,
      last_day:periods[periods.length-1].end_day<q.last_day?periods[periods.length-1].end_day:q.last_day,
      first_period:selected.first,last_period:selected.last};
    let cancelled=false;
    api.costSummary(scoped).then((result)=>{if(!cancelled)selectionCosts=result;})
      .catch((error)=>{if(!cancelled)selectionCostsError=parseError(error);});
    return ()=>{cancelled=true;};
  });

  /** Overview history and trends share period selection; reselecting the same overview point clears it. */
  function onHistoryPeriodClick(label: string): void {
    if (trendSelection?.first === label && trendSelection.last === label) trendSelection = null;
    else onTrendPeriodClick(label);
  }

  const agentsAvailable = $derived(
    Array.from(new Set((catalog?.agents ?? []).map((a) => a.agent))).sort()
  );
  const modelsAvailable = $derived(
    Array.from(new Set((catalog?.models ?? []).map((m) => m.model ?? '__unknown__'))).sort()
  );

  // User options include a synthetic current entry when absent from the list, such as initial default.
  const userOptions = $derived.by(() => {
    const list = [...users];
    if (currentUser && !list.some((u) => u.user_id === currentUser)) {
      list.unshift({ user_id: currentUser, name: currentUser, created_at_ms: 0 });
    }
    return list;
  });

  // Today shares use total_tokens by model/Agent; group unknown names and exclude zero values.
  const todayModelPie = $derived(
    (todayScopeSummary?.models ?? [])
      .map((m) => ({
        name: m.model ?? t('common.unknown'),
        value: m.sums.total_tokens_known === null ? null : Number(m.sums.total_tokens_known),
        input: m.sums.input_total_known === null ? null : Number(m.sums.input_total_known),
        output: m.sums.output_total_known === null ? null : Number(m.sums.output_total_known),
      }))
  );
  const todayAgentPie = $derived(
    (todayScopeSummary?.agents ?? [])
      .map((a) => ({ name: a.agent, value: a.sums.total_tokens_known === null ? null : Number(a.sums.total_tokens_known),
        input: a.sums.input_total_known === null ? null : Number(a.sums.input_total_known),
        output: a.sums.output_total_known === null ? null : Number(a.sums.output_total_known) }))
  );

  // Trend shares use model/Agent total_tokens from the selected-range summary.
  const trendModelPie = $derived(
    (trendScopeSummary?.models ?? [])
      .map((m) => ({
        name: m.model ?? t('common.unknown'),
        value: m.sums.total_tokens_known === null ? null : Number(m.sums.total_tokens_known),
        input: m.sums.input_total_known === null ? null : Number(m.sums.input_total_known),
        output: m.sums.output_total_known === null ? null : Number(m.sums.output_total_known),
      }))
  );
  const trendAgentPie = $derived(
    (trendScopeSummary?.agents ?? [])
      .map((a) => ({ name: a.agent, value: a.sums.total_tokens_known === null ? null : Number(a.sums.total_tokens_known),
        input: a.sums.input_total_known === null ? null : Number(a.sums.input_total_known),
        output: a.sums.output_total_known === null ? null : Number(a.sums.output_total_known) }))
  );

  const trendSessions = $derived(trendScopeSummary?.distinct_sessions ?? null);

  const trendSummaryCards = $derived.by(() => {
    const totals = trendScopeSummary?.totals ?? summary?.totals;
    if (!totals) return [];
    const avgMs = totals.avg_duration_ms === null ? null : Number(totals.avg_duration_ms);
    const cards = [
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
    return trendSelection && !selectionSummary ? cards.map((card) => ({...card,value: selectionError ? '—' : '…',sub: undefined})) : cards;
  });

  /** Date label for the today section. */
  const todayDateLabel = $derived(`${todayDay} · ${settings?.timezone ?? 'UTC'}`);

  /**
   * Selected-period cards mirror today: calls, input/cache/uncached, output, total tokens,
   * cache ratio and sessions; week/month selections also show active days.
   */
  const selectedPeriodCards = $derived.by(() => {
    const cards = [...trendSummaryCards];
    if (granularity === 'week' || granularity === 'month') {
      cards.push({ key: 'activeDays', label: t('cards.activeDays'), value: selectionSummary ? fmtSmart(selectionSummary.active_days) : selectionError ? '—' : '…' });
    }
    return cards;
  });

  function panelTitle(id: string): string {
    switch (id) {
      case 'today-cards':
        return t('overview.today');
      case 'today-hourly':
        return t('hourly.title');
      case 'today-costs':
        return t('cost.title');
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
      case 'trend-costs':
        return t('cost.title');
      case 'trend-weekday':
        return t('trend.weekday');
      case 'trend-model-pie':
        return t('trend.pie.model');
      case 'trend-agent-pie':
        return t('trend.pie.agent');
      case 'trend-model-table':
        return t('table.model');
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

  /** Load the main and today queries together for one interaction/refresh.
   * Preserve object references when query keys and data revisions are unchanged, avoiding
   * idle chart redraws and flicker reported on 2026-09-26. */
  async function loadSummary() {
    if (!ready) return;
    const [q, tq] = [query, todayQuery];
    const scope = JSON.stringify([q, tq, currentUser, settings?.timezone, settings?.week_start, settings?.language, dataReloadKey]);
    if (pendingSummaryKey === scope) return;
    const sequence = ++summarySequence;
    pendingSummaryKey = scope;
    summaryLoading = true;
    queryError = '';
    try {
      const unfiltered = agentFilter || modelFilter
        ? api.summary({ ...q, agents: [], models: [] })
        : null;
      const [s, ts, choices] = await Promise.all([api.summary(q), api.summary(tq), unfiltered]);
      if (sequence !== summarySequence) return;
      const key = `${scope}|${s.data_revision}|${ts.data_revision}`;
      if (key !== loadedDataKey) {
        loadedDataKey = key;
        summary = s;
        todaySummary = ts;
        catalog = choices ?? s;
      }
    } catch (e) {
      if (sequence === summarySequence) queryError = parseError(e);
    } finally {
      if (pendingSummaryKey === scope) pendingSummaryKey = '';
      if (sequence === summarySequence) summaryLoading = false;
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

  /** Reload summary, heatmap and sources after a user switch/import without a changed query. */
  async function reloadUserData() {
    dataReloadKey += 1;
    await Promise.all([loadSummary(), loadSources()]);
  }

  async function onUserChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value;
    if (value === '__new__') {
      // Open user creation while keeping the dropdown on the current user.
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
      ++summarySequence;
      pendingSummaryKey = '';
      summary = todaySummary = catalog = null;
      agentFilter = modelFilter = '';
      currentUser = value;
      userSelect = value;
      // Backend summary/heatmap follow the current user and must reload after switching.
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

  // Collection-status polling and automatic data refresh.
  /** Last observed completed-collection timestamp; a change requires data reload. */
  let lastFinishedMs = 0;
  /** Loaded summary query key and data revision, used to skip identical assignments. */
  let loadedDataKey = '';

  /**
   * Reload data when collection finishes or its completion timestamp changes, including
   * scheduled/headless collection. Idle polling updates status without rebuilding charts,
   * preventing periodic overview flicker.
   */
  let polling = false;
  // An accepted request may not have set the worker's running flag yet.
  let pendingManualFinish = $state<number | null>(null);
  async function pollRefresh() {
    if (polling) return;
    polling = true;
    try {
      const wasRunning = refresh?.running ?? false;
      const r = await api.refreshStatus();
      refresh = r;
      if (pendingManualFinish !== null && (r.running || r.last_finished_ms !== pendingManualFinish)) {
        pendingManualFinish = null;
      }
      if (
        !r.running &&
        r.last_finished_ms > 0 &&
        (wasRunning || lastFinishedMs !== r.last_finished_ms)
      ) {
        lastFinishedMs = r.last_finished_ms;
        if (ready) await Promise.all([loadSummary(), loadSources(), checkTelemetry(true)]);
      }
    } catch {
      /* Retry a failed poll on the next interval. */
    } finally {
      polling = false;
    }
  }

  /**
   * Poll every 500 ms during collection and every 10 seconds when idle.
   * Idle status checks detect system/scheduler collection without redrawing charts.
   */
  const collecting = $derived((refresh?.running ?? false) || pendingManualFinish !== null);
  $effect(() => {
    void collecting;
    const timer = setInterval(() => void pollRefresh(), collecting ? 500 : 10_000);
    return () => clearInterval(timer);
  });

  // Header query-refresh interval: localStorage persistence, default five minutes.
  const AUTO_REFRESH_OPTIONS = [0, 30, 60, 120, 300, 600];
  // The v2 key accompanies the default change from 60 to 300 seconds on 2026-09-26.
  // Ignore the old stored default on first startup with this key.
  const AUTO_REFRESH_KEY = 'llm-usage-auto-refresh-v2';

  function loadAutoRefreshSecs(): number {
    try {
      const value = localStorage.getItem(AUTO_REFRESH_KEY);
      const raw = value === null ? 300 : Number(value);
      return AUTO_REFRESH_OPTIONS.includes(raw) ? raw : 300;
    } catch { return 300; }
  }

  let autoRefreshSecs = $state(loadAutoRefreshSecs());

  function setAutoRefreshSecs(v: number): void {
    autoRefreshSecs = v;
    try {
      localStorage.setItem(AUTO_REFRESH_KEY, String(v));
    } catch {
      /* Without storage access, keep the selection for this session only. */
    }
  }

  function autoRefreshLabel(v: number): string {
    return v < 60
      ? t('header.autoRefresh.sec', { n: v })
      : t('header.autoRefresh.min', { n: v / 60 });
  }

  $effect(() => {
    if (!autoRefreshSecs) return;
    const timer = setInterval(() => void loadSummary(), autoRefreshSecs * 1000);
    return () => clearInterval(timer);
  });

  let refreshing = $state(false);
  async function manualRefresh() {
    refreshing = true;
    try {
      const requested = await api.refreshSources();
      if (requested.running) pendingManualFinish = requested.last_finished_ms;
    } catch (e) {
      loadError = parseError(e);
    } finally {
      refreshing = false;
      await pollRefresh();
    }
  }

  onMount(() => {
    void checkTelemetry();
    let mounted = true;
    void (async () => {
      await Promise.all([loadSettings(), loadUsers(), loadSources(), pollRefresh()]);
      if (mounted) ready = true;
    })();
    const timer = setInterval(() => { clockNow = new Date(); }, 30_000);
    return () => { mounted = false; ++summarySequence; clearInterval(timer); };
  });

  // Today/two-calendar-day ranges select hourly data.
  $effect(() => {
    if (rangeKey === '2' || rangeKey === 'today') granularity = 'hour';
    else if (granularity === 'hour') granularity = 'day';
  });

  // Debounce changed queries by 200 ms and clear a selection whose period label may be stale.
  $effect(() => {
    void query;
    void currentUser;
    void settings?.timezone;
    void settings?.week_start;
    void settings?.language;
    if (!ready) return;
    ++summarySequence;
    pendingSummaryKey = '';
    trendSelection = null;
    const timer = setTimeout(() => void loadSummary(), 200);
    return () => clearTimeout(timer);
  });

  function onSettingsSaved(next: AppSettings) {
    settings = next;
    const locale = normalizeLocale(next.language);
    if (locale) setLocale(locale);
    // Timezone/week-start changes affect date ranges without changing the query key; force reload.
    dataReloadKey += 1;
    // F2: enabling pricing starts a background cost recomputation for retained details.
    // Repeated recomputation is safe; skip when disabled, as collection does not backfill it.
    if (next.pricing?.enabled) {
      void api.recomputeCosts().catch(() => undefined);
    }
  }

  const refreshLabel = $derived(
    collecting
      ? t('action.refreshing')
      : refresh && refresh.last_finished_ms > 0
        ? t('refresh.status.done', {
            time: new Date(refresh.last_finished_ms).toLocaleTimeString(i18n.locale),
            added: refresh.instances.reduce((s, i) => s + i.added, 0),
            updated: refresh.instances.reduce((s, i) => s + i.updated, 0),
          })
        : t('action.refresh')
  );

  // Collection ETA is empty when unavailable so the template hides it.
  const refreshEtaText = $derived(fmtEtaDuration(refresh?.eta_seconds));
</script>

<div class="app-shell">
  <aside class="sidebar">
    <a class="brand" href="#overview" aria-label={t('app.title')} onclick={() => (tab = 'overview')}>
      <img src="/brand/app-icon.svg" width="36" height="36" alt="" />
      <span><strong>{t('app.title')}</strong><small>Usage / Desktop</small></span>
    </a>
    <nav aria-label={t('app.subtitle')}>
      {#each ['overview', 'trend', 'sources', 'details', 'settings'] as id (id)}
        <button class:active={tab === id} aria-label={t('nav.' + id)} aria-current={tab === id ? 'page' : undefined} onclick={() => { if (id === 'settings') settingsSection = 'general'; tab = id as Tab; }}>
          <Icon name={id} /><span>{t('nav.' + id)}</span>
        </button>
      {/each}
    </nav>
    <div class="sidebar-foot"><Icon name="shield" size={18} /><span>{t('workspace.private')}</span></div>
  </aside>
<main>
  <header class="workspace-header">
    <div class="page-heading"><p>{t('workspace.local')}</p><h1>{t('nav.' + tab)}</h1></div>
    <span class="spacer"></span>
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
    <!-- Query-refresh interval is independent of background collection; disabled means manual refresh. -->
    <label class="auto-refresh" title={t('header.autoRefresh.hint')}>
      {t('header.autoRefresh')}
      <select
        value={autoRefreshSecs}
        onchange={(e) => setAutoRefreshSecs(Number((e.currentTarget as HTMLSelectElement).value))}
      >
        {#each AUTO_REFRESH_OPTIONS as v (v)}
          <option value={v}>{v === 0 ? t('header.autoRefresh.off') : autoRefreshLabel(v)}</option>
        {/each}
      </select>
    </label>
    <button class="primary collect-button" title={refreshLabel} disabled={refreshing || collecting} onclick={manualRefresh}>
      <Icon name="refresh" size={17} />{collecting ? t('action.refreshing') : t('action.refresh')}
    </button>
  </header>
  <BudgetReminder {settings} user={currentUser} />
  <div class="workspace-status" role="status">
    <span class="status-dot" class:busy={collecting || summaryLoading}></span>
    {#if refresh?.last_finished_ms}
      <span>{t('workspace.updated', { time: new Date(refresh.last_finished_ms).toLocaleTimeString(i18n.locale) })}</span>
    {:else}<span>{t('workspace.private')}</span>{/if}
    <span>{settings?.timezone ?? 'UTC'}</span>
    {#if summaryLoading}<span>{t('common.loading')}</span>{/if}
  </div>

  {#if loadError}
    <p class="error">{t('common.error', { message: loadError })}</p>
  {/if}
  {#if userError}
    <p class="error">{t('users.actionFailed', { message: userError })}</p>
  {/if}

  {#if tab === 'overview' || tab === 'trend' || tab === 'details'}
    <div class="filters">
      {#if tab === 'trend' || tab === 'details'}
        <label>{t('filter.range')}
          <select bind:value={rangeKey}>
            <option value="2">{t('filter.quick.2days')}</option>
            <option value="today">{t('filter.quick.today')}</option>
            <option value="7">{t('filter.quick.7')}</option>
            <option value="30">{t('filter.quick.30')}</option>
            <option value="365">{t('filter.quick.365')}</option>
          </select>
        </label>
        <label>{t('filter.granularity.label')}
          <select bind:value={granularity}>
            <option value="hour" disabled={rangeKey !== 'today' && rangeKey !== '2'}>{t('filter.granularity.hour')}</option>
            <option value="day">{t('filter.granularity.day')}</option>
            <option value="week">{t('filter.granularity.week')}</option>
            <option value="month">{t('filter.granularity.month')}</option>
          </select>
        </label>
      {/if}
      <label>{t('filter.agent')}
        <select bind:value={agentFilter} aria-label={t('filter.agent')}>
          <option value="">{t('common.all')}</option>
          {#each agentsAvailable as a (a)}
            <option value={a}>{a}</option>
          {/each}
        </select>
      </label>
      <label>{t('filter.model')}
        <select bind:value={modelFilter} aria-label={t('filter.model')}>
          <option value="">{t('common.all')}</option>
          {#each modelsAvailable as m (m)}
            <option value={m === '__unknown__' ? 'unknown' : m}>{m === '__unknown__' ? t('common.unknown') : m}</option>
          {/each}
        </select>
      </label>
      {#if agentFilter || modelFilter}
        <button class="clear-filters" onclick={() => { agentFilter = ''; modelFilter = ''; }}>{t('workspace.filterReset')}</button>
      {/if}
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
          <Icon name="settings" size={16} /> {editLayout ? t('panel.editDone') : t('panel.edit')}
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
    <TelemetrySetup onDetails={() => { settingsSection = 'telemetry'; tab = 'settings'; }} />
    {#if summary}
      <div class="section-head">
        <h2>{t('quota.sectionTitle')}</h2>
      </div>
      <div class="quota-row">
        <QuotaCard agent="copilot" timezone={settings?.timezone ?? 'UTC'} reloadKey={dataReloadKey + (summary?.data_revision ?? 0)} {isDark} />
      </div>
      {#if summary.totals.call_count === 0 && summary.periods.length === 0 && !todaySummary?.periods.length}
        <p class="empty">{t('common.empty')}</p>
      {:else}
        {#if todaySummary}<UsageInsights summary={todayScopeSummary} />{/if}
        <div class="section-head">
          <h2>{t('overview.todaySection')}</h2>
          <span class="section-date">{todayDateLabel}</span>
          {#if todaySelection}<button class="today-range-reset range-reset" onclick={()=>todaySelection=null}>{t('dashboard.resetRange')}</button>{/if}
        </div>
        {#if todaySelection}
          <p class="range-caption today-range-caption">{t('dashboard.selectedRange',{range:todaySelection.first===todaySelection.last?todaySelection.first:`${todaySelection.first} ~ ${todaySelection.last}`})}</p>
          {#if todaySelectionError}<p class="error">{todaySelectionError}</p>{/if}
        {/if}
        <div class="panel-grid">
          {#each todayPanelOrder as id (id)}
            {@const i = panelState.overviewToday.order.indexOf(id)}
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
                  <TodayOverview totals={todayScopeSummary?.totals ?? todaySummary.totals} sessions={todayScopeSummary?.distinct_sessions ?? null} costs={todayScopeCosts} costsError={todayScopeCostsError} pending={todaySelection && !todaySelectionSummary ? todaySelectionError ? 'error' : 'loading' : ''} pricing={settings?.pricing?.enabled ?? false} />
                {:else}
                  <p class="muted">{t('common.loading')}</p>
                {/if}
              {:else if id === 'today-hourly'}
                <p class="period-hint">{t('dashboard.dragHint')}</p>
                <TodayHourly hourly={todaySummary?.today_hourly ?? []} query={todayQuery} {isDark} selectedRange={todaySelection} onrangechange={onTodayRangeChange} />
              {:else if id === 'today-costs'}
                <CostPanel summary={todayScopeCosts} error={todayScopeCostsError} />
                <CostChart summary={costs} periods={todaySummary?.periods ?? []} {isDark} selectedRange={todaySelection} onperiodclick={(label)=>onTodayRangeChange(label,label)} onrangechange={onTodayRangeChange} />
              {:else if id === 'today-model-pie'}
                <SharePie data={todayModelPie} {isDark} />
              {:else if id === 'today-agent-pie'}
                <SharePie data={todayAgentPie} {isDark} />
              {:else if id === 'today-model-table'}
                <BreakdownTables models={todayScopeSummary?.models ?? []} totals={todayScopeSummary?.totals ?? null} costs={todayScopeCosts} pricing={settings?.pricing?.enabled ?? false} kind="model" />
              {:else if id === 'today-agent-table'}
                <BreakdownTables agents={todayScopeSummary?.agents ?? []} kind="agent" />
              {/if}
            </Panel>
          {/each}
        </div>

        <!-- Put range/granularity inside history because today uses an independent query. -->
        <div class="section-head">
          <h2>{t('overview.historySection')}</h2>
          <span class="section-controls" title={t('overview.historyFilterHint')}>
            <label>{t('filter.range')}
              <select bind:value={rangeKey}>
                <option value="2">{t('filter.quick.2days')}</option>
                <option value="today">{t('filter.quick.today')}</option>
                <option value="7">{t('filter.quick.7')}</option>
                <option value="30">{t('filter.quick.30')}</option>
                <option value="365">{t('filter.quick.365')}</option>
              </select>
            </label>
            <label>{t('filter.granularity.label')}
              <select bind:value={granularity}>
                <option value="hour" disabled={rangeKey !== 'today' && rangeKey !== '2'}>{t('filter.granularity.hour')}</option>
                <option value="day">{t('filter.granularity.day')}</option>
                <option value="week">{t('filter.granularity.week')}</option>
                <option value="month">{t('filter.granularity.month')}</option>
              </select>
            </label>
          </span>
        </div>
        <!-- Selecting a historical chart point shows period cards matching the today layout. -->
        {#if trendSelection}
          <div class="period-summary">
            <div class="period-head">
              <span class="period-label">
                <b>{trendRangeCaption}</b>
              </span>
              <button
                type="button"
                class="period-clear"
                onclick={() => (trendSelection = null)}
              >
                × {t('overview.periodSummary.clear')}
              </button>
            </div>
            <div class="summary-cards" class:with-pricing={settings?.pricing?.enabled}>
              {#each selectedPeriodCards as c (c.key)}
                <div class="scard" title={c.hint ?? ''}>
                  <div class="slabel">
                    {c.label}{#if c.hint}<span class="shint">{c.hint}</span>{/if}
                  </div>
                  <div class="svalue">{c.value}</div>
                  {#if c.sub}<div class="ssub">{c.sub}</div>{/if}
                </div>
              {/each}
              {#if settings?.pricing?.enabled}<CostReferenceSummary summary={selectionCosts} error={selectionCostsError} />{/if}
            </div>
            <!-- Model/Agent pies use chart_series with the same selected-period filters.
                 Match loading/chart heights and retain the old chart while replacing data to reduce movement. -->
            {#if selectionError}
              <p class="error">{t('chart.loadFailed', { message: selectionError })}</p>
            {/if}
            {#if selectionSummary}
              <div class="period-pies">
                <div class="ppie">
                  <div class="ppie-title">{t('trend.pie.model')}</div>
                  <SharePie data={trendModelPie} {isDark} height={190} />
                </div>
                <div class="ppie">
                  <div class="ppie-title">{t('trend.pie.agent')}</div>
                  <SharePie data={trendAgentPie} {isDark} height={190} />
                </div>
              </div>
            {:else if !selectionError}
              <div class="period-pies" aria-busy="true">
                <div class="ppie">
                  <div class="ppie-title">{t('trend.pie.model')}</div>
                  <div class="ppie-skeleton"></div>
                </div>
                <div class="ppie">
                  <div class="ppie-title">{t('trend.pie.agent')}</div>
                  <div class="ppie-skeleton"></div>
                </div>
              </div>
            {/if}
          </div>
        {:else}
          <p class="period-hint">{t('overview.periodSummary.hint')}</p>
        {/if}
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
                <CallsChart periods={summary.periods} {query} {granularity} {isDark} selectedRange={trendSelection} onperiodclick={onHistoryPeriodClick} onrangechange={onTrendRangeChange} />
              {:else if id === 'history-tokens'}
                <TokenChart periods={summary.periods} {query} {granularity} {isDark} selectedRange={trendSelection} onperiodclick={onHistoryPeriodClick} onrangechange={onTrendRangeChange} />
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
      <!-- Seven fixed range-summary cards above charts use summary.totals. -->
      <UsageInsights summary={trendScopeSummary} />
      <div class="range-summary">
        <div class="section-head">
          <h2>{t('trend.summary')}</h2>
          {#if trendSelection}<button class="range-reset" onclick={() => trendSelection = null}>{t('dashboard.resetRange')}</button>{/if}
        </div>
        <p class="range-caption">{trendRangeCaption}{#if trendSelection && !selectionSummary && !selectionError} · {t('common.loading')}{/if}</p>
        <p class="period-hint">{t('dashboard.dragHint')}</p>
        {#if selectionError}<p class="error">{selectionError}</p>{/if}
        <div class="summary-cards" class:with-pricing={settings?.pricing?.enabled}>
          {#each trendSummaryCards as c (c.key)}
            <div class="scard" title={[c.label,c.hint,c.sub].filter(Boolean).join(' · ')}>
              <div class="slabel">
                {c.label}
              </div>
              <div class="svalue">{c.value}</div>
            </div>
          {/each}
          {#if settings?.pricing?.enabled}<CostReferenceSummary summary={trendScopeCosts} error={trendSelection ? selectionCostsError : costsError} />{/if}
        </div>
        <div class="summary-quota" aria-label={t('quota.sectionTitle')}>
          <QuotaCard agent="copilot" compact timezone={settings?.timezone ?? 'UTC'} reloadKey={dataReloadKey + (summary?.data_revision ?? 0)} {isDark} />
        </div>
      </div>
      <div class="panel-grid">
        {#each trendPanelOrder as id (id)}
          {@const i = panelState.trendMain.order.indexOf(id)}
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
              <CallsChart periods={summary.periods} {query} {granularity} {isDark} selectedRange={trendSelection} onperiodclick={onTrendPeriodClick} onrangechange={onTrendRangeChange} />
            {:else if id === 'trend-tokens'}
              <TokenChart periods={summary.periods} {query} {granularity} {isDark} selectedRange={trendSelection} onperiodclick={onTrendPeriodClick} onrangechange={onTrendRangeChange} />
            {:else if id === 'trend-costs'}
              <CostPanel summary={trendScopeCosts} error={trendSelection ? selectionCostsError : costsError} />
              <CostChart summary={costs} periods={summary.periods} {isDark} selectedRange={trendSelection} onperiodclick={onTrendPeriodClick} onrangechange={onTrendRangeChange} />
            {:else if id === 'trend-heatmap'}
              <p class="range-caption">{t('dashboard.fullRangePanel')}</p>
              <UsageHeatmap {query} reloadKey={dataReloadKey + (summary?.data_revision ?? 0)} {isDark} />
            {:else if id === 'trend-weekday'}
              <p class="range-caption">{t('dashboard.fullRangePanel')}</p>
              <WeekdayBar {query} reloadKey={dataReloadKey + (summary?.data_revision ?? 0)} {isDark} />
            {:else if id === 'trend-model-pie'}
              <p class="range-caption">{trendRangeCaption}</p>
              {#if trendSelection && !selectionSummary}<p class="muted">{selectionError ? '—' : t('common.loading')}</p>{/if}
              <SharePie data={trendModelPie} {isDark} />
            {:else if id === 'trend-agent-pie'}
              <p class="range-caption">{trendRangeCaption}</p>
              {#if trendSelection && !selectionSummary}<p class="muted">{selectionError ? '—' : t('common.loading')}</p>{/if}
              <SharePie data={trendAgentPie} {isDark} />
            {:else if id === 'trend-model-table'}
              <p class="range-caption">{trendRangeCaption}</p>
              {#if trendScopeSummary}
                <BreakdownTables models={trendScopeSummary.models} totals={trendScopeSummary.totals} costs={trendScopeCosts} pricing={settings?.pricing?.enabled ?? false} kind="model" />
              {:else}<p class="muted">{selectionError ? '—' : t('common.loading')}</p>{/if}
            {/if}
          </Panel>
        {/each}
      </div>
    {:else}
      <p class="muted">{t('common.loading')}</p>
    {/if}
  {:else if tab === 'details'}
    <EventDetails {query} scopeKey={currentUser} timezone={settings?.timezone ?? 'UTC'} reloadKey={dataReloadKey + (summary?.data_revision ?? 0)} />
  {:else if tab === 'sources'}
    <SourceList {sources} users={userOptions} onchanged={reloadUserData} />
  {:else if tab === 'settings'}
    {#if settings}
      <SettingsPanel settings={settings} initialSection={settingsSection} collecting={!!refresh?.running} onsaved={onSettingsSaved} ondatachanged={() => void reloadUserData()} />
    {/if}
  {/if}
</main>
</div>

<style>
  .range-caption {margin: 0 0 10px; font-size: 12px; color: var(--text-muted);}
  .range-reset {border: 1px solid var(--border); background: var(--bg-input); color: var(--accent); border-radius: 6px; padding: 4px 10px; cursor: pointer;}
  .app-shell { min-height: 100vh; display: grid; grid-template-columns: 208px minmax(0, 1fr); }
  .sidebar { position: sticky; top: 0; height: 100vh; display: flex; flex-direction: column; padding: 28px 16px 20px; border-right: 1px solid var(--border); background: var(--bg-card); box-sizing: border-box; }
  .brand { display: flex; gap: 12px; align-items: center; padding: 0 10px; text-decoration: none; color: var(--text-heading); }
  .brand strong { display: block; font-size: 18px; letter-spacing: -0.5px; }
  .brand small { display: block; margin-top: 4px; font-size: 12px; color: var(--text-muted); letter-spacing: 1px; }
  nav { display: flex; flex-direction: column; gap: 7px; margin-top: 42px; }
  nav button { display: flex; gap: 13px; align-items: center; padding: 13px 16px; background: transparent; border: 0; color: var(--text-secondary); text-align: left; border-radius: 10px; font-size: 14px; }
  nav button:hover { background: var(--bg-hover); }
  nav button.active { background: var(--accent-bg); color: var(--accent); font-weight: 650; }
  .sidebar-foot { margin-top: auto; padding: 16px 8px 0; display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text-muted); border-top: 1px solid var(--border-light); }
  main { min-width: 0; padding: 30px 32px 48px; max-width: 2000px; container-type: inline-size; }
  .workspace-header { display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
  .page-heading p { color: var(--text-muted); font-size: 10px; font-weight: 650; letter-spacing: 1.8px; margin: 0 0 8px; }
  h1 { margin: 0; font-size: 28px; letter-spacing: -0.8px; font-weight: 700; color: var(--text-heading); }
  .spacer { flex: 1; }
  .workspace-status { display: flex; gap: 10px; align-items: center; font-size: 12px; color: var(--text-muted); margin: 15px 0 24px; min-height: 18px; }
  .workspace-status span + span + span { margin-left: auto; }
  .status-dot { width: 7px; height: 7px; background: var(--success); border-radius: 50%; }
  .status-dot.busy { background: var(--warning); }
  .collect-button, .edit-toggle { display: inline-flex; gap: 8px; align-items: center; justify-content: center; }
  .clear-filters { color: var(--accent); border: 0; background: transparent; }
  @media (max-width: 1150px) {
    .app-shell { grid-template-columns: 76px minmax(0, 1fr); }
    .sidebar { padding: 24px 10px; }
    .brand { padding: 0 10px; }
    .brand span, nav button span, .sidebar-foot span { display: none; }
    nav button { padding: 14px 17px; }
    .sidebar-foot { justify-content: center; }
    main { padding: 24px; }
  }
  @media (max-width: 700px) {
    .app-shell { display: block; }
    .sidebar { position: static; height: auto; padding: 12px 16px; border-right: 0; border-bottom: 1px solid var(--border); }
    .brand, .sidebar-foot { display: none; }
    nav { margin: 0; flex-direction: row; justify-content: space-between; }
    nav button { padding: 12px; }
    main { padding: 20px 16px; }
    .auto-refresh { display: none; }
    .workspace-header { gap: 12px; }
  }
  .filters {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 14px 16px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    border-radius: 12px;
    margin-bottom: 20px;
    font-size: 13px;
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
  /* Highlight the panel-layout edit toggle when active. */
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
    padding: 8px 10px;
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
  /* Header query-refresh selector shares a row with collection progress and refresh. */
  .auto-refresh {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-secondary);
    white-space: nowrap;
  }
  /* Collection progress width follows its percentage, alongside percentage/ETA text. */
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
    padding: 10px 16px;
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
  /* D8 today/history headings; history range/granularity filters affect that section only. */
  .section-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 28px 0 14px;
    flex-wrap: wrap;
  }
  .quota-row {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 300px));
    gap: 12px;
  }
  .section-head h2 {
    font-size: 17px;
    margin: 0;
    letter-spacing: -0.3px;
    color: var(--text-heading);
  }
  .section-date {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  /* History range/granularity controls sit beside its heading, separate from today. */
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
  /* Selected-period heading and reused summary-cards grid inside history. */
  .period-summary {
    margin: 2px 0 6px;
  }
  .period-head {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-secondary);
    flex-wrap: wrap;
  }
  .period-head .period-label b {
    color: var(--text-heading);
  }
  .period-clear {
    margin-left: auto;
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: 6px;
    padding: 2px 10px;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .period-clear:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .period-hint {
    margin: 2px 0 6px;
    font-size: 12px;
    color: var(--text-muted);
  }
  /* Model/Agent pies use two columns, stacking on narrow screens. */
  .period-pies {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-top: 8px;
  }
  .ppie {
    background: var(--bg-code);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 12px 6px;
    min-width: 0;
    /* Match loading/empty-state height to reduce movement when changing selected periods. */
    min-height: 224px;
    box-sizing: border-box;
  }
  .ppie-title {
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  /* Match the SharePie loading placeholder to its 190px chart height. */
  .ppie-skeleton {
    height: 190px;
    border-radius: 6px;
    background: var(--bg-skeleton);
  }
  /* E10 six-column panel grid; Panel span classes set column widths. */
  .panel-grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    grid-auto-flow: row dense;
    gap: 20px;
    align-items: stretch;
  }
  /* Keep metric rows compact; account quota has an independent full-width strip. */
  .range-summary {
    margin-bottom: 4px;
    container:metrics / inline-size;
  }
  .range-summary .section-head {
    margin: 4px 0 6px;
  }
  .range-summary .summary-cards {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
  .summary-quota { min-width: 0; margin:12px 0; }
  .range-summary .scard {padding:12px 10px;border-radius:9px;}
  .range-summary .svalue {font-size:22px;}
  @container metrics (min-width:1280px) {
    .range-summary .summary-cards {grid-template-columns:repeat(7,minmax(0,1fr));}
    .range-summary .summary-cards.with-pricing {grid-template-columns:repeat(8,minmax(0,1fr));}
  }
  @container metrics (max-width:640px) {
    .range-summary .summary-cards { grid-template-columns: repeat(2,minmax(0,1fr)); }
  }
  @container metrics (max-width:320px) {.range-summary .summary-cards{grid-template-columns:minmax(0,1fr);}}
  .summary-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 8px;
    padding: 8px 0 10px;
  }
  .scard {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px 16px;
    min-width: 0;
  }
  .scard .slabel {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow:hidden;
    text-overflow:ellipsis;
  }
  .scard .slabel .shint {
    color: var(--text-muted);
    font-size: 12px;
  }
  .scard .svalue {
    font-size: 26px;
    font-weight: 650;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }
  .scard .ssub {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
  @media (max-width: 900px) {
    .panel-grid {
      grid-template-columns: 1fr;
    }
    .period-pies {
      grid-template-columns: 1fr;
    }
  }
</style>
