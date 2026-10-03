<script lang="ts">
  import { onMount } from 'svelte';
  import TelemetrySetup from './TelemetrySetup.svelte';
  import { api, parseError } from '../lib/api';
  import type {
    AppSettings,
    ClearAllProgressDto,
    ClearAllPreviewDto,
    CleanupResultDto,
    DiagnosticLogRowDto,
    ExportFilterOptionsDto,
    PriceSnapshotInfoDto,
    PriceRefreshOutcomeDto,
    PriceRefreshStatusDto,
    RetentionTiers,
    StorageStatsDto,
    SummaryQuery,
    SystemTaskStatusDto,
  } from '../lib/api';
  import { i18n, t, fmtBytes, fmtNumber, LANGUAGE_OPTIONS } from '../lib/i18n.svelte';

  let {
    settings,
    onsaved,
    ondatachanged,
    collecting = false,
    initialSection = 'general',
  }: {
    settings: AppSettings;
    onsaved: (next: AppSettings) => void;
    /** 导入等改变底层数据的操作完成后通知父级刷新查询。 */
    ondatachanged?: () => void;
    /** 采集进行中（父级轮询 refresh_status），期间禁用“清理全部数据”。 */
    collecting?: boolean;
    initialSection?: 'general' | 'telemetry';
  } = $props();

  type SubTab = 'general' | 'telemetry' | 'retention' | 'costs' | 'system' | 'identity' | 'export' | 'logs';
  type RetentionField = 'events' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'yearly';

  let sub = $state<SubTab>('general');
  $effect(() => { sub = initialSection; });

  // svelte-ignore state_referenced_locally
  // 草稿编辑器刻意只捕获挂载时的设置初值；外部更新由父组件重新挂载本面板。
  let draft = $state<AppSettings>({
    ...settings,
    manual_roots: [...settings.manual_roots],
    retention: { ...settings.retention },
    pricing: {
      enabled: settings.pricing?.enabled ?? false,
      provider_defaults: (settings.pricing?.provider_defaults ?? []).map((d) => ({ ...d })),
      online_refresh_enabled: settings.pricing?.online_refresh_enabled ?? false,
      online_cache_ttl_days: settings.pricing?.online_cache_ttl_days ?? 3,
    },
  });
  // svelte-ignore state_referenced_locally
  // 文本输入统一走字符串草稿（年为空 = 终身），保存时再解析校验。
  let inputs = $state({
    weekStart: settings.week_start === null ? '' : String(settings.week_start),
    events: String(settings.retention.events_days),
    hourly: String(settings.retention.hourly_days),
    daily: String(settings.retention.daily_days),
    weekly: String(settings.retention.weekly_days),
    monthly: String(settings.retention.monthly_days),
    yearly: settings.retention.yearly_days === null ? '' : String(settings.retention.yearly_days),
    alias: settings.hostname_alias ?? '',
    refreshTtl: String(settings.pricing?.online_cache_ttl_days ?? 3),
  });
  // svelte-ignore state_referenced_locally
  let rootsText = $state(settings.manual_roots.join('\n'));

  let saving = $state(false);
  let message = $state('');
  let errorMessage = $state('');

  let info = $state<{ db_path: string; host_id: string; schema_version: number } | null>(null);
  let taskStatus = $state<SystemTaskStatusDto | null>(null);
  let taskLoading = $state(true);
  let taskError = $state('');
  let autoBusy = $state(false);
  let taskBusy = $state(false);
  let systemMessage = $state('');
  let systemError = $state('');
  let exportMessage = $state('');
  let exportError = $state('');

  // 导出过滤（任务 G）：用户/主机多选（默认当前用户/当前主机），全选 = 不过滤。
  let filterOptions = $state<ExportFilterOptionsDto | null>(null);
  let selectedUsers = $state<string[]>([]);
  let selectedHosts = $state<string[]>([]);

  // 存储统计与手动清理。
  let stats = $state<StorageStatsDto | null>(null);
  let statsError = $state('');
  let cleanupDays = $state('30');
  let cleaning = $state(false);
  let cleanupMessage = $state('');
  let cleanupError = $state('');

  // 清理全部数据并重新采集（确认层 + 后台阶段进度 + 结果/重采提示）。
  let clearAllOpen = $state(false);
  let clearAllBusy = $state(false);
  let clearAllMessage = $state('');
  let clearAllError = $state('');
  /** 后台任务当前阶段（空 = 未运行）；文案键 cleanup.clearAllPhase.<phase>。 */
  let clearAllPhase = $state('');
  /** cleared 阶段送达的各表清除计数与备份路径（done 时汇总展示）。 */
  let clearAllCleared = $state<Record<string, number> | null>(null);
  let clearAllBackupPath = $state<string | null>(null);
  /** 清空预检（打开确认层时加载）：磁盘已不存在的源文件 = 清空后无法重采。 */
  let clearPreview = $state<ClearAllPreviewDto | null>(null);

  onMount(() => {
    let unlisten: (() => void) | null = null;
    // 后台清理任务的阶段事件 → 确认层实时进度；done/failed 收尾。
    const handleProgress = (p: ClearAllProgressDto) => {
      if (p.phase === 'failed') {
        clearAllPhase = '';
        clearAllBusy = false;
        clearAllError = t('cleanup.failed', { message: p.error ?? '' });
        return;
      }
      if (p.phase === 'cleared') {
        clearAllCleared = p.cleared ?? null;
        clearAllBackupPath = p.backup ?? null;
        // 清空结果立即反映到存储统计与界面（重采完成后 done 再刷新一次）。
        void loadStats();
        ondatachanged?.();
      }
      if (p.phase === 'done') {
        clearAllPhase = '';
        clearAllBusy = false;
        clearAllMessage = t('cleanup.clearAllDone', {
          detail: clearAllDetail(clearAllCleared ?? {}),
        });
        if (clearAllBackupPath) {
          clearAllMessage = `${clearAllMessage} ${t('cleanup.clearAllBackupAt', { path: clearAllBackupPath })}`;
        }
        clearAllMessage = `${clearAllMessage} ${t('cleanup.clearAllTriggered')}`;
        // 存储统计与界面数据反映清空+重采结果（父级轮询结束时还会再刷一次）。
        void loadStats();
        ondatachanged?.();
        clearAllOpen = false;
        return;
      }
      clearAllPhase = p.phase;
    };
    void api.onClearAllProgress(handleProgress).then((f) => (unlisten = f));
    return () => {
      unlisten?.();
      stopRefreshPoll();
    };
  });

  // zcode db 历史回填（滚动窗口源文件丢失的恢复路径）。

  // 聚合交换包导入。
  let importing = $state(false);
  let importMessage = $state('');
  let importError = $state('');

  // 诊断日志（日志 Tab）：最近 200 条，支持手动/自动刷新 + code 过滤。
  let logs = $state<DiagnosticLogRowDto[]>([]);
  let logsLoading = $state(false);
  let logsError = $state('');
  let logsAuto = $state(false);
  /** code 下拉过滤（'' = 全部；选中后传 code_filter 给 diagnostic_logs）。 */
  let logsCode = $state('');
  /** code 选项列表（未过滤加载时从返回行提取 DISTINCT code）。 */
  let logsCodes = $state<string[]>([]);
  /** 快捷开关：客户端隐藏 expired_by_retention 保留清理行。 */
  let hideRetention = $state(false);

  const RETENTION_CODE = 'expired_by_retention';
  /** 保留清理行占比过半时显示“隐藏保留清理”快捷按钮（已开启时保留按钮供还原）。 */
  const retentionDominant = $derived(
    logs.length > 0 && logs.filter((r) => r.code === RETENTION_CODE).length > logs.length / 2
  );
  const displayLogs = $derived(
    hideRetention ? logs.filter((r) => r.code !== RETENTION_CODE) : logs
  );

  /** 常见 IANA 时区（纯下拉选择；上方搜索框过滤长列表）。 */
  const TIMEZONES: string[] = [
    'UTC',
    'Asia/Shanghai', 'Asia/Hong_Kong', 'Asia/Taipei', 'Asia/Tokyo', 'Asia/Seoul',
    'Asia/Singapore', 'Asia/Kuala_Lumpur', 'Asia/Bangkok', 'Asia/Jakarta', 'Asia/Manila',
    'Asia/Ho_Chi_Minh', 'Asia/Kolkata', 'Asia/Karachi', 'Asia/Dhaka', 'Asia/Dubai',
    'Asia/Riyadh', 'Asia/Tehran', 'Asia/Jerusalem', 'Asia/Almaty', 'Asia/Tashkent',
    'Asia/Yekaterinburg', 'Asia/Novosibirsk', 'Asia/Vladivostok',
    'Europe/London', 'Europe/Dublin', 'Europe/Lisbon', 'Europe/Paris', 'Europe/Berlin',
    'Europe/Amsterdam', 'Europe/Brussels', 'Europe/Zurich', 'Europe/Vienna', 'Europe/Prague',
    'Europe/Warsaw', 'Europe/Stockholm', 'Europe/Oslo', 'Europe/Copenhagen', 'Europe/Helsinki',
    'Europe/Madrid', 'Europe/Rome', 'Europe/Athens', 'Europe/Bucharest', 'Europe/Moscow',
    'Europe/Istanbul',
    'Africa/Cairo', 'Africa/Lagos', 'Africa/Nairobi', 'Africa/Johannesburg',
    'America/New_York', 'America/Toronto', 'America/Chicago', 'America/Mexico_City',
    'America/Denver', 'America/Phoenix', 'America/Los_Angeles', 'America/Vancouver',
    'America/Santiago', 'America/Sao_Paulo', 'America/Argentina/Buenos_Aires',
    'Australia/Sydney', 'Australia/Melbourne', 'Australia/Brisbane',
    'Pacific/Auckland',
  ];

  // 时区选择（2026-09-26 改造）：默认只显示当前值的“伪 select”按钮，点击弹出
  // absolute 覆盖层（搜索框 + 过滤后的选项列表）；选择/点击外部/Esc 关闭。
  let tzFilter = $state('');
  let tzOpen = $state(false);
  /** 覆盖层根节点（click-outside 命中测试用；bind:this 赋值）。 */
  let tzRoot = $state<HTMLElement | null>(null);
  const tzOptions = $derived.by(() => {
    const needle = tzFilter.trim().toLowerCase();
    const list = needle === '' ? [...TIMEZONES] : TIMEZONES.filter((z) => z.toLowerCase().includes(needle));
    if (draft.timezone && !list.includes(draft.timezone)) list.unshift(draft.timezone);
    return list;
  });

  function openTz(): void {
    tzFilter = '';
    tzOpen = true;
  }

  function pickTz(z: string): void {
    draft.timezone = z;
    tzOpen = false;
  }

  /** 覆盖层打开时聚焦搜索框（Svelte action，元素插入即执行）。 */
  function focusInput(node: HTMLInputElement): void {
    node.focus();
  }

  // 当前查询（导出用）：近 30 天日粒度。
  const exportQuery: SummaryQuery = {
    first_day: new Date(Date.now() - 29 * 86_400_000).toISOString().slice(0, 10),
    last_day: new Date().toISOString().slice(0, 10),
    granularity: 'day',
    agents: [],
    providers: [],
    models: [],
  };

  const subTabs = $derived.by(() => [
    ['general', t('settings.tab.general')],
    ['telemetry', t('telemetry.title')],
    ['retention', t('settings.tab.retention')],
    ['costs', t('settings.tab.costs')],
    ['system', t('settings.tab.system')],
    ['identity', t('settings.tab.identity')],
    ['export', t('settings.tab.export')],
    ['logs', t('settings.tab.logs')],
  ] as [SubTab, string][]);

  const retentionRows = $derived.by(() => [
    { field: 'events', label: t('settings.retention.events'), hint: t('settings.retention.defaultDays', { days: 7 }) },
    { field: 'hourly', label: t('settings.retention.hourly'), hint: t('settings.retention.defaultDays', { days: 3 }) },
    { field: 'daily', label: t('settings.retention.daily'), hint: t('settings.retention.defaultDays', { days: 90 }) },
    { field: 'weekly', label: t('settings.retention.weekly'), hint: t('settings.retention.defaultDays', { days: 1095 }) },
    { field: 'monthly', label: t('settings.retention.monthly'), hint: t('settings.retention.defaultDays', { days: 3650 }) },
    { field: 'yearly', label: t('settings.retention.yearly'), hint: t('settings.retention.yearlyForever') },
  ] as { field: RetentionField; label: string; hint: string }[]);

  // 周起始生效值（与后端 effective_week_start 同规则：显式优先；zh→周一，
  // en-US/CA→周日，其余周一）。
  const effectiveWeekStart = $derived.by(() => {
    if (inputs.weekStart === '6') return 6;
    if (inputs.weekStart === '0') return 0;
    const lang = draft.language.toLowerCase();
    if (lang.startsWith('zh')) return 0;
    if (lang.startsWith('en') && (lang.includes('us') || lang.includes('ca'))) return 6;
    return 0;
  });

  async function loadInfo() {
    try {
      info = await api.appInfo();
    } catch {
      info = null;
    }
  }

  async function loadTaskStatus() {
    taskLoading = true;
    taskError = '';
    try {
      taskStatus = await api.systemTaskStatus();
    } catch (e) {
      taskStatus = null;
      taskError = parseError(e);
    } finally {
      taskLoading = false;
    }
  }

  async function loadStats() {
    statsError = '';
    try {
      stats = await api.storageStats();
    } catch (e) {
      stats = null;
      statsError = t('cleanup.statsFailed', { message: parseError(e) });
    }
  }

  // ---- F2 费用估算：价格快照与供应商默认 ----
  let snapshots = $state<PriceSnapshotInfoDto[]>([]);
  let snapshotError = $state('');
  let costMessage = $state('');
  let costError = $state('');
  let recomputeBusy = $state(false);

  async function loadSnapshots() {
    snapshotError = '';
    try {
      snapshots = await api.listPriceSnapshots();
    } catch (e) {
      snapshots = [];
      snapshotError = parseError(e);
    }
  }

  function addProviderDefault() {
    if (!draft.pricing) draft.pricing = { enabled: false, provider_defaults: [] };
    draft.pricing.provider_defaults = [
      ...draft.pricing.provider_defaults,
      { provider_id: '', region: '', channel: '', cache_ttl_minutes: null },
    ];
  }

  function removeProviderDefault(index: number) {
    if (!draft.pricing) return;
    draft.pricing.provider_defaults = draft.pricing.provider_defaults.filter((_, i) => i !== index);
  }

  async function importSnapshot() {
    costMessage = '';
    costError = '';
    try {
      const path = await api.pickOpenPath('json');
      if (!path) return;
      const r = await api.importPriceSnapshot(path);
      costMessage = r.already_present
        ? `${r.snapshot_id} (${r.inserted_rows})`
        : `${r.snapshot_id} → ${r.inserted_rows}`;
      await loadSnapshots();
    } catch (e) {
      costError = parseError(e);
    }
  }

  async function recomputeCosts() {
    if (recomputeBusy) return;
    recomputeBusy = true;
    costMessage = '';
    costError = '';
    try {
      await api.recomputeCosts();
      costMessage = t('cost.settings.recomputeStarted');
    } catch (e) {
      costError = parseError(e);
    } finally {
      recomputeBusy = false;
    }
  }

  // ---- F2 在线刷新（models.dev）：状态轮询与手动刷新 ----
  let refreshStatus = $state<PriceRefreshStatusDto | null>(null);
  let refreshBusy = $state(false);
  let refreshPoll: ReturnType<typeof setInterval> | null = null;

  async function loadRefreshStatus() {
    try {
      refreshStatus = await api.priceRefreshStatus();
    } catch {
      refreshStatus = null;
    }
  }

  function stopRefreshPoll() {
    if (refreshPoll !== null) {
      clearInterval(refreshPoll);
      refreshPoll = null;
    }
  }

  function refreshOutcomeText(o: PriceRefreshOutcomeDto): string {
    const date = o.cache ? fmtSnapshotDate(o.cache.fetched_at_ms) : '—';
    switch (o.status) {
      case 'fetched':
        return t('cost.refresh.result.fetched', { id: o.snapshot_id ?? '—', rows: o.inserted_rows });
      case 'cache_fresh':
        return t('cost.refresh.result.cacheFresh', { date });
      case 'fetch_failed_used_cache':
        return t('cost.refresh.result.usedCache', { date, message: o.error ?? '' });
      default:
        return t('cost.refresh.result.noCache', { message: o.error ?? '' });
    }
  }

  async function refreshPricesNow() {
    if (refreshBusy) return;
    refreshBusy = true;
    costMessage = '';
    costError = '';
    try {
      await api.refreshPricesOnline(true);
      stopRefreshPoll();
      refreshPoll = setInterval(() => {
        void (async () => {
          await loadRefreshStatus();
          if (refreshStatus && !refreshStatus.running) {
            stopRefreshPoll();
            refreshBusy = false;
            if (refreshStatus.last_outcome) {
              costMessage = refreshOutcomeText(refreshStatus.last_outcome);
            }
            await loadSnapshots();
            ondatachanged?.();
          }
        })();
      }, 1000);
      await loadRefreshStatus();
      if (refreshStatus && !refreshStatus.running) {
        // 极快完成（或未能启动）：直接收尾，不依赖轮询。
        stopRefreshPoll();
        refreshBusy = false;
        if (refreshStatus.last_outcome) {
          costMessage = refreshOutcomeText(refreshStatus.last_outcome);
        }
        await loadSnapshots();
        ondatachanged?.();
      }
    } catch (e) {
      costError = parseError(e);
      refreshBusy = false;
    }
  }

  function fmtSnapshotDate(ms: number): string {
    return ms > 0 ? new Date(ms).toISOString().slice(0, 10) : '—';
  }

  /** 导出过滤选项：默认勾选当前用户/当前主机（失败时多选框退化为仅当前值）。 */
  async function loadExportFilters() {
    try {
      const r = await api.exportFilterOptions();
      filterOptions = r;
      const curUser = r.users.find((u) => u.is_current)?.user_id ?? r.current_user;
      const curHost = r.hosts.find((h) => h.is_current)?.host_id ?? r.current_host;
      selectedUsers = [curUser];
      selectedHosts = [curHost];
    } catch {
      filterOptions = null;
      selectedUsers = [];
      selectedHosts = [];
    }
  }
  loadInfo();
  loadTaskStatus();
  loadStats();
  loadExportFilters();
  loadSnapshots();
  loadRefreshStatus();

  /** 解析单级保留天数：undefined = 非法；null = 留空（仅年允许 = 终身）。 */
  function tierValue(raw: string, allowEmpty: boolean): number | null | undefined {
    const s = raw.trim();
    if (s === '') return allowEmpty ? null : undefined;
    const n = Number(s);
    if (!Number.isInteger(n) || n < 1) return undefined;
    return n;
  }

  function fail(msg: string): void {
    errorMessage = t('settings.saveFailed', { message: msg });
    saving = false;
  }

  /**
   * 常规页恢复默认（仅改草稿，保存后生效）：语言 zh-CN、主题跟随系统、
   * 时区系统值、周起始自动、采集间隔 3600。手工根目录是用户数据源清单，
   * 不属于偏好默认，不清空。
   */
  function restoreGeneralDefaults(): void {
    draft.language = 'zh-CN';
    draft.theme = 'system';
    draft.timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
    inputs.weekStart = '';
    draft.refresh_interval_secs = 3600;
    errorMessage = '';
    message = t('settings.defaultsPending');
  }

  /** 归档保留页恢复默认（仅改草稿，保存后生效）：7/3/90/1095/3650 天，年 = 空（终身）。 */
  function restoreRetentionDefaults(): void {
    inputs.events = '7';
    inputs.hourly = '3';
    inputs.daily = '90';
    inputs.weekly = '1095';
    inputs.monthly = '3650';
    inputs.yearly = '';
    errorMessage = '';
    message = t('settings.defaultsPending');
  }

  // 保存仅提交常规/归档/来源身份类字段；开机与后台即时生效，不经此路径。
  async function save() {
    saving = true;
    message = '';
    errorMessage = '';
    const timezone = draft.timezone.trim();
    if (timezone === '') {
      fail(t('settings.invalidTimezone'));
      return;
    }
    const interval = draft.refresh_interval_secs;
    if (
      typeof interval !== 'number' ||
      !Number.isInteger(interval) ||
      interval < 0 ||
      interval > 86_400
    ) {
      fail(t('settings.invalidInterval'));
      return;
    }
    const tiers: Partial<Record<RetentionField, number | null>> = {};
    for (const field of ['events', 'hourly', 'daily', 'weekly', 'monthly', 'yearly'] as RetentionField[]) {
      const v = tierValue(inputs[field], field === 'yearly');
      if (v === undefined) {
        fail(t('settings.invalidNumber'));
        return;
      }
      tiers[field] = v;
    }
    // 校验通过后各级必有值；?? 兜底仅满足类型（yearly null = 终身）。
    const retention: RetentionTiers = {
      events_days: tiers.events ?? 0,
      hourly_days: tiers.hourly ?? 0,
      daily_days: tiers.daily ?? 0,
      weekly_days: tiers.weekly ?? 0,
      monthly_days: tiers.monthly ?? 0,
      yearly_days: tiers.yearly ?? null,
    };
    const alias = inputs.alias.trim();
    const refreshTtl = Number(inputs.refreshTtl.trim());
    if (!Number.isInteger(refreshTtl) || refreshTtl < 1 || refreshTtl > 365) {
      fail(t('settings.invalidNumber'));
      return;
    }
    const pricing = draft.pricing
      ? { ...draft.pricing, online_cache_ttl_days: refreshTtl }
      : draft.pricing;
    const next: AppSettings = {
      ...draft,
      timezone,
      refresh_interval_secs: interval,
      week_start: inputs.weekStart === '6' ? 6 : inputs.weekStart === '0' ? 0 : null,
      retention,
      manual_roots: rootsText.split('\n').map((l) => l.trim()).filter(Boolean),
      hostname_alias: alias === '' ? null : alias,
      pricing,
    };
    try {
      await api.setSettings(next);
      message = t('settings.saved');
      onsaved(next);
    } catch (e) {
      errorMessage = t('settings.saveFailed', { message: parseError(e) });
    } finally {
      saving = false;
    }
  }

  async function toggleAutoStart() {
    if (!taskStatus || taskStatus.unsupported || autoBusy) return;
    autoBusy = true;
    systemMessage = '';
    systemError = '';
    try {
      await api.setAutoStart(!taskStatus.auto_start);
      systemMessage = t('system.applied');
    } catch (e) {
      systemError = t('system.actionFailed', { message: parseError(e) });
    } finally {
      autoBusy = false;
      // 无论成败都重读状态，让开关回到系统真实值。
      await loadTaskStatus();
    }
  }

  async function toggleRefreshTask() {
    if (!taskStatus || taskStatus.unsupported || taskBusy) return;
    taskBusy = true;
    systemMessage = '';
    systemError = '';
    try {
      await api.setRefreshTask(!(taskStatus.refresh_task_desired ?? taskStatus.refresh_task));
      systemMessage = t('system.applied');
    } catch (e) {
      systemError = t('system.actionFailed', { message: parseError(e) });
    } finally {
      taskBusy = false;
      await loadTaskStatus();
    }
  }

  /** 保存对话框路径 → 所在目录（后端在该目录下生成本次导出文件名）。 */
  function parentDir(path: string): string {
    const i = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
    return i > 0 ? path.slice(0, i) : path;
  }

  /**
   * 多选 → 后端单值过滤的展开：全部勾选 = null（不过滤，等价合并一份）；
   * 单选 = 该值；子集多选 = 逐个值分别导出（交换包文件名含时间戳不互相覆盖）。
   */
  function expandSelection(all: string[], selected: string[]): (string | null)[] {
    if (all.length > 0 && selected.length === all.length) return [null];
    return [...selected];
  }

  /** 至少勾选一个用户和一个主机才允许导出。 */
  const canExport = $derived(selectedUsers.length > 0 && selectedHosts.length > 0);

  function selectAllUsers(): void {
    if (filterOptions) selectedUsers = filterOptions.users.map((u) => u.user_id);
  }

  function selectAllHosts(): void {
    if (filterOptions) selectedHosts = filterOptions.hosts.map((h) => h.host_id);
  }

  async function exportData(kind: 'summary-csv' | 'exchange') {
    exportMessage = '';
    exportError = '';
    const defaultName =
      kind === 'summary-csv'
        ? `usage-${exportQuery.first_day}-${exportQuery.last_day}.csv`
        : 'exchange.json';
    let picked: string | null;
    try {
      picked = await api.pickSavePath(defaultName);
    } catch (e) {
      exportError = t('export.failed', { message: parseError(e) });
      return;
    }
    if (picked === null) {
      exportMessage = t('export.cancelled');
      return;
    }
    try {
      if (kind === 'summary-csv') {
        // CSV 为展示用汇总，后端不按用户/主机过滤，单次导出即可。
        const r = await api.exportData(kind, parentDir(picked), exportQuery, null, null);
        exportMessage = t('export.done', { path: r.path });
        return;
      }
      // 交换包：按勾选的用户 × 主机组合展开（全选合并为一份 null 过滤）。
      const userScopes = expandSelection(filterOptions?.users.map((u) => u.user_id) ?? [], selectedUsers);
      const hostScopes = expandSelection(filterOptions?.hosts.map((h) => h.host_id) ?? [], selectedHosts);
      const paths: string[] = [];
      for (const u of userScopes) {
        for (const h of hostScopes) {
          const r = await api.exportData('exchange', parentDir(picked), exportQuery, u, h);
          paths.push(r.path);
          // 交换包文件名含毫秒时间戳，错开 2ms 防止同毫秒覆盖。
          await new Promise((res) => setTimeout(res, 2));
        }
      }
      exportMessage =
        paths.length === 1
          ? t('export.done', { path: paths[0] })
          : t('export.doneMulti', {
              count: paths.length,
              paths: paths.join(i18n.locale === 'zh-CN' ? '；' : '; '),
            });
    } catch (e) {
      exportError = t('export.failed', { message: parseError(e) });
    }
  }

  async function importData() {
    importMessage = '';
    importError = '';
    let picked: string | null;
    try {
      picked = await api.pickOpenPath('json');
    } catch (e) {
      importError = t('import.failed', { message: parseError(e) });
      return;
    }
    if (picked === null) {
      importMessage = t('import.cancelled');
      return;
    }
    importing = true;
    try {
      const r = await api.importExchange(picked);
      importMessage = t('import.done', {
        sources: r.sources_registered,
        dailyInserted: r.daily_inserted,
        dailyReplaced: r.daily_replaced,
        dailySkipped: r.daily_skipped,
        dailyConflicts: r.daily_conflicts,
        hourlyInserted: r.hourly_inserted,
        hourlyReplaced: r.hourly_replaced,
        hourlySkipped: r.hourly_skipped,
      });
      ondatachanged?.();
    } catch (e) {
      importError = t('import.failed', { message: parseError(e) });
    } finally {
      importing = false;
    }
  }

  /** 诊断日志时间：格式化到秒（本机时区，24 小时制）。 */
  function fmtLogTime(ms: number): string {
    return new Date(ms).toLocaleString(i18n.locale, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    });
  }

  async function loadLogs() {
    logsLoading = true;
    // 同步读取以建立 effect 依赖（code 过滤变化时在 Tab 内自动重查）。
    const code = logsCode || null;
    try {
      const r = await api.diagnosticLogs(200, code);
      logs = r.rows;
      // 未过滤加载时刷新 code 选项列表（过滤加载保留旧列表供切换）。
      if (!code) logsCodes = Array.from(new Set(r.rows.map((x) => x.code))).sort();
      logsError = '';
    } catch (e) {
      logsError = t('logs.failed', { message: parseError(e) });
    } finally {
      logsLoading = false;
    }
  }

  // 进入日志 Tab 或切换 code 过滤时（重新）加载。
  $effect(() => {
    if (sub !== 'logs') return;
    void logsCode;
    void loadLogs();
  });
  $effect(() => {
    if (sub !== 'logs' || !logsAuto) return;
    const timer = setInterval(() => void loadLogs(), 10_000);
    return () => clearInterval(timer);
  });

  async function runCleanup() {
    cleanupMessage = '';
    cleanupError = '';
    const n = Number(cleanupDays.trim());
    if (!Number.isInteger(n) || n < 1) {
      cleanupError = t('cleanup.invalidDays');
      return;
    }
    cleaning = true;
    try {
      const r: CleanupResultDto = await api.manualCleanup(n);
      cleanupMessage = t('cleanup.result', {
        events: r.deleted_events,
        hourly: r.deleted_hourly_rows,
        daily: r.deleted_daily_rows,
        period: r.deleted_period_rows,
        materialized: r.materialized_period_rows,
      });
      await loadStats();
    } catch (e) {
      cleanupError = t('cleanup.failed', { message: parseError(e) });
    } finally {
      cleaning = false;
    }
  }

  /** clear_all_data 各表名 → 展示标签（核心表用现有键；其余保留原名）。 */
  const CLEAR_ALL_TABLE_LABELS: Record<string, string> = {
    usage_events: 'cleanup.stats.events',
    hourly_usage: 'cleanup.stats.hourly',
    daily_usage: 'cleanup.stats.daily',
    period_usage: 'cleanup.stats.period',
    diagnostics: 'cleanup.stats.diagnostics',
  };

  function clearAllDetail(cleared: Record<string, number>): string {
    const sep = i18n.locale === 'zh-CN' ? '、' : ', ';
    return Object.entries(cleared)
      .map(([table, n]) => {
        const label = CLEAR_ALL_TABLE_LABELS[table];
        return `${label ? t(label) : table} ${fmtNumber(n)}`;
      })
      .join(sep);
  }

  function openClearAll() {
    clearAllMessage = '';
    clearAllError = '';
    clearAllOpen = true;
    // 预检尽力而为：失败不阻塞确认层（此时不展示缺失文件预警）。
    clearPreview = null;
    void api
      .clearAllPreview()
      .then((p) => (clearPreview = p))
      .catch(() => (clearPreview = null));
  }

  /** 清理全部数据 → 后台执行（确认层实时显示阶段；完成展示各表清除条目数）。
   * 命令立即返回，清库/备份/全量重采在后台线程进行（同步执行会冻结 UI）；
   * 进度经 clear-all-progress 事件更新确认层，重采百分比另见顶栏。 */
  async function confirmClearAll() {
    clearAllBusy = true;
    clearAllError = '';
    clearAllMessage = '';
    clearAllCleared = null;
    clearAllBackupPath = null;
    clearAllPhase = 'waiting';
    try {
      const r = await api.clearAllData();
      if (!r.started) {
        // 已有任务在执行：留在确认层跟随其阶段事件。
        return;
      }
    } catch (e) {
      clearAllPhase = '';
      clearAllBusy = false;
      clearAllError = t('cleanup.failed', { message: parseError(e) });
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (clearAllOpen && !clearAllBusy && e.key === 'Escape') clearAllOpen = false;
    if (tzOpen && e.key === 'Escape') tzOpen = false;
  }}
  onpointerdown={(e) => {
    // 时区覆盖层点击外部关闭（命中层内元素不关）。
    if (tzOpen && tzRoot && !tzRoot.contains(e.target as Node)) tzOpen = false;
  }}
/>

<form onsubmit={(e) => { e.preventDefault(); void save(); }}>
  <div class="settings-layout">
    <nav class="sidebar">
      {#each subTabs as [id, label] (id)}
        <button type="button" data-settings-section={id} class:active={sub === id} onclick={() => (sub = id)}>{label}</button>
      {/each}
    </nav>

    <div class="content">
      {#if sub === 'general'}
        <section class="panel">
          <div class="frow">
            <span class="flabel">{t('settings.language')}</span>
            <div class="fvalue">
              <select bind:value={draft.language} aria-label={t('settings.language')} data-testid="language-select">
                {#each LANGUAGE_OPTIONS as [code, label] (code)}
                  <option value={code}>{label}</option>
                {/each}
              </select>
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('settings.theme')}</span>
            <div class="fvalue">
              <div class="theme-seg" role="radiogroup" aria-label={t('settings.theme')}>
                {#each ['system', 'light', 'dark'] as opt (opt)}
                  <button
                    type="button"
                    class:active={draft.theme === opt}
                    role="radio"
                    aria-checked={draft.theme === opt}
                    onclick={() => (draft.theme = opt)}
                  >
                    {t(`settings.theme.${opt}`)}
                  </button>
                {/each}
              </div>
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('settings.timezone')}</span>
            <div class="fvalue">
              <div class="tz-picker" bind:this={tzRoot}>
                <button
                  type="button"
                  class="tz-select"
                  aria-haspopup="listbox"
                  aria-expanded={tzOpen}
                  onclick={() => (tzOpen ? (tzOpen = false) : openTz())}
                >
                  <span class="mono">{draft.timezone}</span>
                  <span class="tz-caret" aria-hidden="true">▾</span>
                </button>
                {#if tzOpen}
                  <div class="tz-overlay" role="listbox">
                    <input
                      class="tz-search"
                      placeholder={t('settings.timezone.search')}
                      bind:value={tzFilter}
                      spellcheck="false"
                      use:focusInput
                      onkeydown={(e) => {
                        if (e.key === 'Escape') {
                          e.stopPropagation();
                          tzOpen = false;
                        }
                      }}
                    />
                    <div class="tz-list">
                      {#each tzOptions as z (z)}
                        <button
                          type="button"
                          class="tz-option"
                          class:active={z === draft.timezone}
                          role="option"
                          aria-selected={z === draft.timezone}
                          onclick={() => pickTz(z)}
                        >
                          {z}
                        </button>
                      {:else}
                        <p class="tz-empty">{t('settings.timezone.noMatch')}</p>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('settings.weekStart')}</span>
            <div class="fvalue">
              <select bind:value={inputs.weekStart}>
                <option value="">{t('settings.weekStart.auto')}</option>
                <option value="0">{t('settings.weekStart.monday')}</option>
                <option value="6">{t('settings.weekStart.sunday')}</option>
              </select>
              {#if inputs.weekStart === ''}
                <span class="hint">{t('settings.weekStart.autoEffective', {
                  value: effectiveWeekStart === 6 ? t('settings.weekStart.sunday') : t('settings.weekStart.monday'),
                })}</span>
              {/if}
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('settings.interval')}</span>
            <div class="fvalue">
              <input type="number" min="0" max="86400" bind:value={draft.refresh_interval_secs} />
              <span class="hint">{t('settings.interval.hint')}</span>
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('settings.restoreDefaults')}</span>
            <div class="fvalue">
              <button type="button" class="mini" onclick={restoreGeneralDefaults}>
                {t('settings.restoreDefaults')}
              </button>
            </div>
          </div>
          <div class="frow top">
            <span class="flabel">{t('settings.manualRoots')}</span>
            <div class="fvalue">
              <textarea bind:value={rootsText} rows="3" spellcheck="false"></textarea>
            </div>
          </div>
          <p class="note">{t('settings.note')}</p>
        </section>
      {:else if sub === 'telemetry'}
        <TelemetrySetup full />
      {:else if sub === 'retention'}
        <section class="panel">
          <h4>{t('cleanup.stats')}</h4>
          {#if stats}
            <div class="stats">
              <span class="chip"><span class="k">{t('cleanup.stats.events')}</span><span class="v">{fmtNumber(stats.events)}</span></span>
              <span class="chip"><span class="k">{t('cleanup.stats.hourly')}</span><span class="v">{fmtNumber(stats.hourly)}</span></span>
              <span class="chip"><span class="k">{t('cleanup.stats.daily')}</span><span class="v">{fmtNumber(stats.daily)}</span></span>
              <span class="chip"><span class="k">{t('cleanup.stats.period')}</span><span class="v">{fmtNumber(stats.period)}</span></span>
              <span class="chip"><span class="k">{t('cleanup.stats.diagnostics')}</span><span class="v">{fmtNumber(stats.diagnostics)}</span></span>
              <span class="chip"><span class="k">{t('cleanup.stats.size')}</span><span class="v">{fmtBytes(stats.db_bytes + stats.wal_bytes)}</span></span>
            </div>
          {:else if statsError}
            <p class="bad">{statsError}</p>
          {:else}
            <p class="hint">{t('common.loading')}</p>
          {/if}
          {#each retentionRows as row (row.field)}
            <div class="tier">
              <span class="name">{row.label}</span>
              <span class="hint">{row.hint}</span>
              <input inputmode="numeric" placeholder="—" bind:value={inputs[row.field]} />
              <span class="unit">{t('settings.retention.days')}</span>
            </div>
          {/each}
          <div class="frow">
            <span class="flabel">{t('settings.restoreDefaults')}</span>
            <div class="fvalue">
              <button type="button" class="mini" onclick={restoreRetentionDefaults}>
                {t('settings.restoreDefaults')}
              </button>
            </div>
          </div>
          <p class="warning">{t('settings.retention.warning')}</p>
        </section>
        <section class="panel">
          <h4>{t('cleanup.title')}</h4>
          <div class="frow">
            <span class="flabel">{t('cleanup.daysBefore')}</span>
            <div class="fvalue">
              <input class="days" inputmode="numeric" bind:value={cleanupDays} />
              <span class="unit">{t('cleanup.daysUnit')}</span>
              <button type="button" class="danger" disabled={cleaning} onclick={() => void runCleanup()}>
                {cleaning ? t('cleanup.running') : t('cleanup.run')}
              </button>
            </div>
          </div>
          {#if cleanupMessage}<p class="ok">{cleanupMessage}</p>{/if}
          {#if cleanupError}<p class="bad">{cleanupError}</p>{/if}
        </section>
        <section class="panel">
          <div class="clear-all-row">
            <button
              type="button"
              class="danger solid"
              disabled={clearAllBusy || collecting}
              onclick={openClearAll}
            >
              {clearAllBusy ? t('cleanup.running') : t('cleanup.clearAll')}
            </button>
            <span class="hint">{t('cleanup.clearAllConfirm')}</span>
          </div>
          {#if clearAllMessage}<p class="ok">{clearAllMessage}</p>{/if}
          {#if clearAllError}<p class="bad">{clearAllError}</p>{/if}
        </section>
        <p class="hint">{t('settings.archives.hint')}</p>
      {:else if sub === 'costs'}
        <section class="panel">
          <h4>{t('cost.title')}</h4>
          <div class="frow">
            <label class="flabel" for="cost-enabled">{t('cost.settings.enabled')}</label>
            <div class="fvalue">
              <input
                id="cost-enabled"
                class="switch"
                role="switch"
                type="checkbox"
                checked={draft.pricing?.enabled ?? false}
                onchange={(e) => {
                  if (!draft.pricing) draft.pricing = { enabled: false, provider_defaults: [] };
                  draft.pricing.enabled = e.currentTarget.checked;
                }}
              />
            </div>
          </div>
          <p class="note">{t('cost.settings.hint')}</p>
        </section>
        <section class="panel">
          <h4>{t('cost.settings.snapshots')}</h4>
          {#if snapshots.length === 0 && !snapshotError}
            <p class="hint">{t('common.empty')}</p>
          {:else}
            <table class="snap-table">
              <tbody>
                {#each snapshots as s (s.snapshot_id)}
                  <tr>
                    <td>{s.snapshot_id}</td>
                    <td>{s.source_type}</td>
                    <td>{fmtSnapshotDate(s.fetched_at_ms)} · {s.row_count}</td>
                    <td>{s.verified_by ?? '—'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
          {#if snapshotError}<p class="bad">{snapshotError}</p>{/if}
          <div class="frow">
            <div class="fvalue">
              <button type="button" class="mini" onclick={() => void importSnapshot()}>
                {t('cost.settings.import')}
              </button>
              <button
                type="button"
                class="mini"
                disabled={recomputeBusy}
                onclick={() => void recomputeCosts()}
              >
                {t('cost.settings.recompute')}
              </button>
            </div>
          </div>
          {#if costMessage}<p class="ok">{costMessage}</p>{/if}
          {#if costError}<p class="bad">{costError}</p>{/if}
        </section>
        <section class="panel">
          <h4>{t('cost.refresh.title')}</h4>
          <div class="frow">
            <label class="flabel" for="price-refresh-enabled">{t('cost.refresh.enable')}</label>
            <div class="fvalue">
              <input
                id="price-refresh-enabled"
                class="switch"
                role="switch"
                type="checkbox"
                checked={draft.pricing?.online_refresh_enabled ?? false}
                onchange={(e) => {
                  if (!draft.pricing) draft.pricing = { enabled: false, provider_defaults: [] };
                  draft.pricing.online_refresh_enabled = e.currentTarget.checked;
                }}
              />
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('cost.refresh.ttl')}</span>
            <div class="fvalue">
              <input class="num" bind:value={inputs.refreshTtl} spellcheck="false" />
            </div>
          </div>
          <div class="frow">
            <div class="fvalue">
              <button
                type="button"
                class="mini"
                disabled={refreshBusy || (refreshStatus !== null && !refreshStatus.enabled)}
                onclick={() => void refreshPricesNow()}
              >
                {refreshBusy ? t('cost.refresh.running') : t('cost.refresh.now')}
              </button>
            </div>
          </div>
          <p class="hint">
            {refreshStatus?.cache
              ? t('cost.refresh.cacheLine', {
                  date: fmtSnapshotDate(refreshStatus.cache.fetched_at_ms),
                  days: Math.floor(refreshStatus.cache.age_secs / 86400),
                })
              : t('cost.refresh.never')}
            {#if refreshStatus?.last_outcome}
              {' · '}{refreshOutcomeText(refreshStatus.last_outcome)}
            {/if}
          </p>
          <p class="note">{t('cost.refresh.hint')}</p>
        </section>
        <section class="panel">
          <h4>{t('cost.settings.addProvider')}</h4>
          {#each draft.pricing?.provider_defaults ?? [] as d, i}
            <div class="tier">
              <input placeholder={t('cost.settings.provider')} bind:value={d.provider_id} />
              <input placeholder={t('cost.settings.region')} bind:value={d.region} />
              <input placeholder={t('cost.settings.channel')} bind:value={d.channel} />
              <select
                value={d.cache_ttl_minutes === null || d.cache_ttl_minutes === undefined ? '' : String(d.cache_ttl_minutes)}
                onchange={(e) => {
                  const v = e.currentTarget.value;
                  d.cache_ttl_minutes = v === '' ? null : Number(v);
                }}
              >
                <option value="">—</option>
                <option value="5">5m</option>
                <option value="60">1h</option>
              </select>
              <button type="button" class="mini" onclick={() => removeProviderDefault(i)}>
                {t('cost.settings.remove')}
              </button>
            </div>
          {/each}
          <div class="frow">
            <div class="fvalue">
              <button type="button" class="mini" onclick={addProviderDefault}>
                {t('cost.settings.addProvider')}
              </button>
            </div>
          </div>
        </section>
      {:else if sub === 'system'}
        <section class="panel">
          {#if taskLoading}
            <p class="hint">{t('system.loading')}</p>
          {:else if !taskStatus}
            <p class="bad">{t('system.actionFailed', { message: taskError })}</p>
          {:else if taskStatus.unsupported}
            <p class="hint">{t('system.unsupported')}</p>
          {:else}
            <div class="sysrow">
              <div class="sysinfo">
                <span class="name">{t('system.autoStart')} · {taskStatus.auto_start ? t('system.state.on') : t('system.state.off')}</span>
                <span class="hint">{t('system.autoStart.hint')}</span>
              </div>
              <input
                class="switch"
                type="checkbox"
                role="switch"
                checked={taskStatus.auto_start}
                disabled={autoBusy}
                onchange={() => void toggleAutoStart()}
              />
            </div>
            <div class="sysrow">
              <div class="sysinfo">
                <span class="name">{t('system.refreshTask')} · {taskStatus.refresh_task ? t('system.refreshTask.installed') : t('system.refreshTask.notInstalled')}</span>
                <span class="hint">{t('system.refreshTask.hint')}</span>
                {#if taskStatus.refresh_task}
                  <span class="hint">{t('system.refreshTask.interval')}</span>
                {/if}
                <span class="hint">{t('system.refreshTask.desired')} · {(taskStatus.refresh_task_desired ?? taskStatus.refresh_task) ? t('system.state.on') : t('system.state.off')}</span>
                {#if taskStatus.refresh_task_error}
                  <span class="bad">{t('system.actionFailed', {message: parseError(taskStatus.refresh_task_error)})}</span>
                {/if}
              </div>
              <button type="button" disabled={taskBusy} onclick={() => void toggleRefreshTask()}>
                {(taskStatus.refresh_task_desired ?? taskStatus.refresh_task) ? t('system.refreshTask.uninstall') : t('system.refreshTask.install')}
              </button>
              {#if taskStatus.refresh_task_error}
                <button type="button" disabled={taskBusy} onclick={async () => {
                  taskBusy = true;
                  try { await api.setRefreshTask(taskStatus?.refresh_task_desired ?? false); }
                  catch (e) { systemError = parseError(e); }
                  finally { taskBusy = false; await loadTaskStatus(); }
                }}>{t('system.refreshTask.repair')}</button>
              {/if}
            </div>
          {/if}
          {#if systemMessage}<p class="ok">{systemMessage}</p>{/if}
          {#if systemError}<p class="bad">{systemError}</p>{/if}
        </section>
      {:else if sub === 'identity'}
        <section class="panel">
          {#if info}
            <dl class="meta">
              <dt>{t('settings.host')}</dt>
              <dd class="mono">{info.host_id}</dd>
              <dt>{t('settings.hostnameAlias')}</dt>
              <dd>
                <input bind:value={inputs.alias} placeholder="—" spellcheck="false" />
                <span class="hint">{t('settings.hostnameAlias.hint')}</span>
              </dd>
              <dt>{t('settings.dbPath')}</dt>
              <dd class="mono">{info.db_path}</dd>
              <dt>{t('settings.schemaVersion')}</dt>
              <dd class="mono">v{info.schema_version}</dd>
            </dl>
          {:else}
            <p class="hint">{t('common.loading')}</p>
          {/if}
        </section>
      {:else if sub === 'export'}
        <section class="panel">
          <h4>{t('export.scopeTitle')}</h4>
          {#if filterOptions}
            <div class="export-filters">
              <div class="escope">
                <div class="escope-head">
                  <span class="escope-title">{t('export.userFilter')}</span>
                  <button type="button" class="mini" onclick={selectAllUsers}>{t('export.selectAll')}</button>
                </div>
                <div class="echecks">
                  {#each filterOptions.users as u (u.user_id)}
                    <label class="echeck">
                      <input type="checkbox" value={u.user_id} bind:group={selectedUsers} />
                      <span class="ename">{u.name || u.user_id}</span>
                      {#if u.is_current}<span class="etag">{t('export.currentTag')}</span>{/if}
                    </label>
                  {/each}
                </div>
              </div>
              <div class="escope">
                <div class="escope-head">
                  <span class="escope-title">{t('export.hostFilter')}</span>
                  <button type="button" class="mini" onclick={selectAllHosts}>{t('export.selectAll')}</button>
                </div>
                <div class="echecks">
                  {#each filterOptions.hosts as h (h.host_id)}
                    <label class="echeck">
                      <input type="checkbox" value={h.host_id} bind:group={selectedHosts} />
                      <span class="ename">{h.name || h.host_id}</span>
                      {#if h.is_current}<span class="etag">{t('export.currentTag')}</span>{/if}
                    </label>
                  {/each}
                </div>
              </div>
            </div>
            <p class="hint">{t('export.scopeHint')}</p>
          {:else}
            <p class="hint">{t('common.loading')}</p>
          {/if}
          <div class="export">
            <button type="button" disabled={!canExport} onclick={() => void exportData('summary-csv')}>{t('export.csv')}</button>
            <button type="button" disabled={!canExport} onclick={() => void exportData('exchange')}>{t('export.exchange')}</button>
          </div>
          {#if filterOptions && !canExport}<p class="bad">{t('export.noneSelected')}</p>{/if}
          {#if exportMessage}<p class="ok">{exportMessage}</p>{/if}
          {#if exportError}<p class="bad">{exportError}</p>{/if}
        </section>
        <!-- 导入与导出用分隔线隔开（.panel + .panel 顶边框）；导入不受导出范围影响。 -->
        <section class="panel">
          <h4>{t('import.title')}</h4>
          <p class="hint">{t('import.scopeHint')}</p>
          <div class="export">
            <button type="button" disabled={importing} onclick={() => void importData()}>
              {importing ? t('common.loading') : t('import.button')}
            </button>
          </div>
          {#if importMessage}<p class="ok">{importMessage}</p>{/if}
          {#if importError}<p class="bad">{importError}</p>{/if}
        </section>
      {:else if sub === 'logs'}
        <section class="panel">
          <div class="logs-toolbar">
            <h4>{t('logs.title')}</h4>
            <label class="logs-filter">
              {t('logs.code')}
              <select bind:value={logsCode}>
                <option value="">{t('common.all')}</option>
                {#each logsCodes as c (c)}
                  <option value={c}>{c}</option>
                {/each}
              </select>
            </label>
            {#if retentionDominant || hideRetention}
              <button type="button" class="mini" onclick={() => (hideRetention = !hideRetention)}>
                {hideRetention ? t('logs.showRetention') : t('logs.hideRetention')}
              </button>
            {/if}
            <span class="logs-spacer"></span>
            <label class="logs-auto">
              <input
                class="switch"
                type="checkbox"
                role="switch"
                bind:checked={logsAuto}
              />
              {t('logs.auto')}
            </label>
            <button type="button" disabled={logsLoading} onclick={() => void loadLogs()}>
              {logsLoading ? t('common.loading') : t('logs.refresh')}
            </button>
          </div>
          {#if logsError}
            <p class="bad">{logsError}</p>
          {/if}
          {#if logsLoading && displayLogs.length === 0}
            <p class="hint">{t('common.loading')}</p>
          {:else if displayLogs.length === 0 && !logsError}
            <p class="hint">{t('logs.empty')}</p>
          {:else}
            <div class="logs-table-wrap">
              <table class="logs-table">
                <thead>
                  <tr>
                    <th>{t('logs.time')}</th>
                    <th>{t('logs.code')}</th>
                    <th>{t('logs.field')}</th>
                    <th>{t('logs.instance')}</th>
                    <th>{t('logs.message')}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each displayLogs as row, i (i)}
                    <tr>
                      <td class="mono nowrap">{fmtLogTime(row.time)}</td>
                      <td class="mono">{row.code}</td>
                      <td class="mono">{row.field ?? '—'}</td>
                      <td class="mono">{row.instance ?? '—'}</td>
                      <td class="logs-msg">{row.message}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </section>
      {/if}

      {#if sub === 'general' || sub === 'retention' || sub === 'costs' || sub === 'identity'}
        <div class="save-row">
          <button type="submit" class="primary" disabled={saving}>{t('settings.save')}</button>
          {#if message}<span class="ok">{message}</span>{/if}
          {#if errorMessage}<span class="bad">{errorMessage}</span>{/if}
        </div>
      {/if}
    </div>
  </div>

  {#if clearAllOpen}
    <div class="overlay">
      <div class="dialog" role="dialog" aria-modal="true" aria-label={t('cleanup.clearAll')}>
        <p class="dialog-text">{t('cleanup.clearAllConfirm')}</p>
        {#if clearPreview && clearPreview.missing_files > 0}
          <p class="dialog-warn">{t('cleanup.clearAllMissing', { n: clearPreview.missing_files })}</p>
        {/if}
        <p class="dialog-note">{t('cleanup.clearAllBackup')}</p>
        {#if clearAllBusy}
          <p class="dialog-note" role="status" aria-live="polite">
            {#if clearAllPhase}
              {t(`cleanup.clearAllPhase.${clearAllPhase}`)}
            {:else}
              {t('cleanup.running')}
            {/if}
          </p>
          <div class="dialog-progress" aria-hidden="true"></div>
        {/if}
        {#if clearAllError}
          <p class="dialog-warn">{clearAllError}</p>
        {/if}
        <div class="dialog-actions">
          <button
            type="button"
            class="danger solid"
            disabled={clearAllBusy}
            onclick={() => void confirmClearAll()}
          >
            {clearAllBusy ? t('cleanup.running') : t('cleanup.clearAll')}
          </button>
          <button type="button" disabled={clearAllBusy} onclick={() => (clearAllOpen = false)}>
            {t('users.createCancel')}
          </button>
        </div>
      </div>
    </div>
  {/if}
</form>

<style>
  /* Typora 风格：左侧竖向分类菜单 + 右侧内容表单。 */
  .snap-table td {
    padding: 4px 10px 4px 0;
    border-bottom: 1px solid var(--line, rgba(128, 128, 128, 0.25));
    text-align: left;
  }
  .settings-layout {
    display: grid;
    grid-template-columns: 160px minmax(0, 1fr);
    gap: 0 20px;
    align-items: start;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 10px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    border-right: 1px solid var(--border);
    position: sticky;
    top: 0;
  }
  .sidebar button {
    border: none;
    background: transparent;
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    text-align: left;
    color: var(--text-secondary);
  }
  .sidebar button:hover {
    background: var(--bg-hover);
  }
  .sidebar button.active {
    background: var(--bg-nav-active);
    color: var(--accent);
    font-weight: 600;
  }
  .content {
    min-width: 0;
    padding: 26px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
  }
  .panel {
    padding: 4px 0 10px;
  }
  .panel + .panel {
    border-top: 1px solid var(--border-light);
    margin-top: 6px;
    padding-top: 10px;
  }
  .panel h4 {
    font-size: 16px;
    margin: 2px 0 20px;
    color: var(--text-heading);
  }
  /* 紧凑表单行：行距 10px、标签 140px 右对齐。 */
  .frow {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 18px;
    min-width: 0;
  }
  .frow.top {
    align-items: flex-start;
  }
  .theme-seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .theme-seg button {
    padding: 5px 14px;
    font-size: 13px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }
  .theme-seg button + button {
    border-left: 1px solid var(--border);
  }
  .theme-seg button.active {
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 500;
  }
  .theme-seg button:not(.active):hover {
    background: var(--bg-hover);
  }
  .flabel {
    flex: none;
    width: 140px;
    text-align: left;
    font-size: 13px;
    color: var(--text-secondary);
  }
  .fvalue {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    flex-wrap: wrap;
  }
  input, select, textarea {
    padding: 8px 10px;
    font-size: 13px;
    box-sizing: border-box;
  }
  .fvalue > input, .fvalue > select {
    width: 240px;
  }
  .fvalue > textarea {
    flex: 1;
    min-width: 240px;
    font-family: ui-monospace, monospace;
  }
  .note {
    font-size: 12px;
    color: var(--text-muted);
    margin: 2px 0 0 150px;
  }
  .hint {
    font-size: 12px;
    color: var(--text-muted);
  }
  .name {
    font-size: 13px;
    color: var(--text);
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 10px;
  }
  .chip {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    background: var(--bg-code);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 5px 10px;
    font-size: 12px;
  }
  .chip .k {
    color: var(--text-secondary);
  }
  .chip .v {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }
  .tier {
    display: grid;
    grid-template-columns: 140px 1fr 110px max-content;
    gap: 8px;
    align-items: center;
    padding: 5px 0;
    margin-bottom: 5px;
    border-bottom: 1px solid var(--border-light);
  }
  .tier .name {
    text-align: right;
  }
  .tier input {
    width: 100%;
    text-align: right;
  }
  .unit {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .days {
    width: 90px !important;
    text-align: right;
  }
  .warning {
    font-size: 12px;
    color: var(--warning);
    margin: 8px 0 0;
  }
  .sysrow {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border-light);
  }
  .sysinfo {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sysinfo .name {
    font-size: 13px;
    color: var(--text);
  }
  .meta {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 12px;
    font-size: 13px;
    background: var(--bg-code);
    border-radius: 8px;
    padding: 10px 14px;
    align-items: center;
  }
  .meta dt {
    color: var(--text-secondary);
  }
  .mono {
    font-family: ui-monospace, monospace;
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  /* 时区选择：伪 select 按钮 + 点击展开的搜索覆盖层（absolute，点击外部关闭）。 */
  .tz-picker {
    position: relative;
    width: 240px;
  }
  .tz-select {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 4px 8px;
    font-size: 13px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
  }
  .tz-select:hover {
    border-color: var(--accent);
  }
  .tz-caret {
    color: var(--text-muted);
    font-size: 10px;
  }
  .tz-overlay {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    width: 280px;
    background: var(--bg-modal);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(16, 24, 40, 0.14);
    z-index: 60;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .tz-overlay .tz-search {
    width: 100%;
  }
  .tz-list {
    max-height: 240px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tz-option {
    text-align: left;
    border: none;
    background: transparent;
    padding: 4px 8px;
    font-size: 12px;
    border-radius: 6px;
    cursor: pointer;
    font-family: ui-monospace, monospace;
    color: var(--text);
  }
  .tz-option:hover {
    background: var(--bg-hover);
  }
  .tz-option.active {
    background: var(--accent-bg);
    color: var(--accent);
    font-weight: 600;
  }
  .tz-empty {
    margin: 0;
    padding: 6px 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .export {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  /* 日志 Tab：工具行 + 可滚动表格（时间到秒，mono 等宽）。 */
  .logs-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .logs-toolbar h4 {
    margin: 0;
  }
  .logs-spacer {
    flex: 1;
  }
  /* code 过滤下拉（“全部” + DISTINCT code 选项）。 */
  .logs-filter {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-secondary);
  }
  .logs-filter select {
    padding: 3px 6px;
    font-size: 12px;
    font-family: ui-monospace, monospace;
    max-width: 220px;
  }
  .logs-auto {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .logs-table-wrap {
    max-height: 440px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .logs-table {
    border-collapse: collapse;
    width: 100%;
    font-size: 12px;
  }
  .logs-table th,
  .logs-table td {
    text-align: left;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border-light);
    vertical-align: top;
  }
  .logs-table th {
    position: sticky;
    top: 0;
    background: var(--bg-code);
    color: var(--text-secondary);
    font-weight: 600;
    white-space: nowrap;
  }
  .logs-table tbody tr:last-child td {
    border-bottom: none;
  }
  .logs-table .mono {
    font-family: ui-monospace, monospace;
    font-size: 11.5px;
    white-space: nowrap;
  }
  .logs-table .nowrap {
    white-space: nowrap;
  }
  .logs-table .logs-msg {
    overflow-wrap: anywhere;
    min-width: 220px;
  }
  /* 导出范围（任务 G + 多选改造）：用户/主机两组 checkbox（每组带“全选”）。 */
  .export-filters {
    display: flex;
    gap: 24px;
    flex-wrap: wrap;
    margin-bottom: 6px;
  }
  .escope {
    min-width: 220px;
    max-width: 340px;
    flex: 1;
  }
  .escope-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 4px;
  }
  .escope-title {
    font-size: 13px;
    color: var(--text);
    font-weight: 600;
  }
  button.mini {
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: 6px;
    padding: 2px 10px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  button.mini:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .echecks {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 180px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 10px;
  }
  .echeck {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }
  .echeck input {
    accent-color: var(--accent);
    margin: 0;
  }
  .echeck .ename {
    overflow-wrap: anywhere;
  }
  .echeck .etag {
    color: var(--text-muted);
    font-size: 12px;
    white-space: nowrap;
  }
  button {
    padding: 5px 14px;
    cursor: pointer;
    font-size: 13px;
  }
  button.primary {
    background: var(--accent);
    color: var(--accent-text);
    border: none;
    border-radius: 6px;
  }
  button.primary:disabled {
    background: var(--accent);
    opacity: 0.55;
    cursor: wait;
  }
  button.danger {
    color: var(--danger);
    border: 1px solid var(--danger);
    background: var(--bg-input);
    border-radius: 6px;
  }
  button.danger:disabled {
    cursor: wait;
    opacity: 0.6;
  }
  /* 危险操作的实心红样式（清理全部数据）。 */
  button.danger.solid {
    background: var(--danger);
    color: var(--accent-text);
    border-color: var(--danger);
  }
  .clear-all-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  /* 确认层：居中模态（Esc/取消关闭；执行中不可关）。 */
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(28, 30, 33, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog {
    background: var(--bg-modal);
    border-radius: 10px;
    padding: 18px 20px;
    max-width: 480px;
    box-shadow: var(--shadow);
    border: 1px solid var(--border);
  }
  .dialog-text {
    font-size: 13px;
    color: var(--text);
    margin: 0;
  }
  /* 清空确认层：缺失源文件预警（危险色）与自动备份说明（弱化）。 */
  .dialog-warn {
    font-size: 13px;
    color: var(--danger);
    margin: 8px 0 0;
  }
  .dialog-note {
    font-size: 13px;
    color: var(--text-muted);
    margin: 8px 0 0;
  }
  /* 后台清理阶段的流动指示（不定进度；百分比语义见顶栏采集进度）。 */
  .dialog-progress {
    margin-top: 10px;
    height: 3px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
    position: relative;
  }
  .dialog-progress::after {
    content: '';
    position: absolute;
    inset: 0 auto 0 0;
    width: 35%;
    border-radius: 2px;
    background: var(--accent);
    animation: dialog-progress-slide 1.2s ease-in-out infinite;
  }
  @keyframes dialog-progress-slide {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(320%);
    }
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 14px;
  }
  .save-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-top: 8px;
  }
  .ok {
    color: var(--success);
    font-size: 13px;
  }
  .bad {
    color: var(--danger);
    font-size: 13px;
  }
  .ok, .bad {
    overflow-wrap: anywhere;
  }
  @media (max-width: 700px) {
    .settings-layout {
      grid-template-columns: 1fr;
    }
    .sidebar {
      flex-direction: row;
      border-right: none;
      border-bottom: 1px solid var(--border);
      flex-wrap: wrap;
    }
    /* 窄屏退化为常规堆叠，避免 140px 标签挤压输入。 */
    .frow {
      flex-wrap: wrap;
    }
    .flabel {
      width: auto;
      text-align: left;
    }
    .tier {
      grid-template-columns: 1fr 110px max-content;
    }
    .tier .name {
      text-align: left;
    }
    .tier .hint {
      grid-column: 1 / -1;
    }
    .note {
      margin-left: 0;
    }
  }
</style>
