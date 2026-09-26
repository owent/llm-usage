<script lang="ts">
  import { api, parseError } from '../lib/api';
  import type {
    AppSettings,
    ClearAllDataResultDto,
    CleanupResultDto,
    DiagnosticLogRowDto,
    ExportFilterOptionsDto,
    RetentionTiers,
    StorageStatsDto,
    SummaryQuery,
    SystemTaskStatusDto,
  } from '../lib/api';
  import { i18n, t, fmtBytes, fmtNumber } from '../lib/i18n.svelte';

  let {
    settings,
    onsaved,
    ondatachanged,
    collecting = false,
  }: {
    settings: AppSettings;
    onsaved: (next: AppSettings) => void;
    /** 导入等改变底层数据的操作完成后通知父级刷新查询。 */
    ondatachanged?: () => void;
    /** 采集进行中（父级轮询 refresh_status），期间禁用“清理全部数据”。 */
    collecting?: boolean;
  } = $props();

  type SubTab = 'general' | 'retention' | 'system' | 'identity' | 'export' | 'logs';
  type RetentionField = 'events' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'yearly';

  let sub = $state<SubTab>('general');

  // svelte-ignore state_referenced_locally
  // 草稿编辑器刻意只捕获挂载时的设置初值；外部更新由父组件重新挂载本面板。
  let draft = $state<AppSettings>({
    ...settings,
    manual_roots: [...settings.manual_roots],
    retention: { ...settings.retention },
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

  // 导出过滤（任务 G）：用户/主机下拉，默认当前用户/当前主机。
  let filterOptions = $state<ExportFilterOptionsDto | null>(null);
  let userFilter = $state('');
  let hostFilter = $state('');

  // 存储统计与手动清理。
  let stats = $state<StorageStatsDto | null>(null);
  let statsError = $state('');
  let cleanupDays = $state('30');
  let cleaning = $state(false);
  let cleanupMessage = $state('');
  let cleanupError = $state('');

  // 清理全部数据并重新采集（确认层 + 结果/重采提示）。
  let clearAllOpen = $state(false);
  let clearAllBusy = $state(false);
  let clearAllMessage = $state('');
  let clearAllError = $state('');

  // 聚合交换包导入。
  let importing = $state(false);
  let importMessage = $state('');
  let importError = $state('');

  // 诊断日志（日志 Tab）：最近 200 条，支持手动/自动刷新。
  let logs = $state<DiagnosticLogRowDto[]>([]);
  let logsLoading = $state(false);
  let logsError = $state('');
  let logsAuto = $state(false);
  let logsLoadedOnce = $state(false);

  /** 常见 IANA 时区（datalist 可输入筛选；自由输入仍允许）。 */
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
    ['retention', t('settings.tab.retention')],
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
    { field: 'monthly', label: t('settings.retention.monthly'), hint: t('settings.retention.defaultDays', { days: 10950 }) },
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

  /** 导出过滤选项：is_current 项默认选中（失败时下拉退化为仅当前值）。 */
  async function loadExportFilters() {
    try {
      const r = await api.exportFilterOptions();
      filterOptions = r;
      userFilter = r.users.find((u) => u.is_current)?.user_id ?? r.current_user;
      hostFilter = r.hosts.find((h) => h.is_current)?.host_id ?? r.current_host;
    } catch {
      filterOptions = null;
    }
  }
  loadInfo();
  loadTaskStatus();
  loadStats();
  loadExportFilters();

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
    const next: AppSettings = {
      ...draft,
      timezone,
      refresh_interval_secs: interval,
      week_start: inputs.weekStart === '6' ? 6 : inputs.weekStart === '0' ? 0 : null,
      retention,
      manual_roots: rootsText.split('\n').map((l) => l.trim()).filter(Boolean),
      hostname_alias: alias === '' ? null : alias,
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
      await api.setRefreshTask(!taskStatus.refresh_task);
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
      const r = await api.exportData(
        kind,
        parentDir(picked),
        exportQuery,
        userFilter || null,
        hostFilter || null
      );
      exportMessage = t('export.done', { path: r.path });
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
    try {
      const r = await api.diagnosticLogs(200);
      logs = r.rows;
      logsError = '';
    } catch (e) {
      logsError = t('logs.failed', { message: parseError(e) });
    } finally {
      logsLoading = false;
    }
  }

  // 首次进入日志 Tab 时加载一次；自动刷新开启时每 10 秒重拉（仅 Tab 激活期间）。
  $effect(() => {
    if (sub === 'logs' && !logsLoadedOnce) {
      logsLoadedOnce = true;
      void loadLogs();
    }
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
  }

  /** 清理全部数据 → 展示各表清除条目数 → 自动触发全量重采（进度见顶栏）。 */
  async function confirmClearAll() {
    clearAllBusy = true;
    clearAllError = '';
    try {
      const r: ClearAllDataResultDto = await api.clearAllData();
      clearAllMessage = t('cleanup.clearAllDone', { detail: clearAllDetail(r.cleared) });
      // 存储统计与界面数据立即反映清空结果。
      await loadStats();
      ondatachanged?.();
      // 游标已重置，本轮刷新即全量重新采集。
      await api.refreshSources();
      clearAllMessage = `${clearAllMessage} ${t('cleanup.clearAllTriggered')}`;
    } catch (e) {
      clearAllError = t('cleanup.failed', { message: parseError(e) });
    } finally {
      clearAllBusy = false;
      clearAllOpen = false;
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (clearAllOpen && !clearAllBusy && e.key === 'Escape') clearAllOpen = false;
  }}
/>

<form onsubmit={(e) => { e.preventDefault(); void save(); }}>
  <div class="settings-layout">
    <nav class="sidebar">
      {#each subTabs as [id, label] (id)}
        <button type="button" class:active={sub === id} onclick={() => (sub = id)}>{label}</button>
      {/each}
    </nav>

    <div class="content">
      {#if sub === 'general'}
        <section class="panel">
          <div class="frow">
            <span class="flabel">{t('settings.language')}</span>
            <div class="fvalue">
              <select bind:value={draft.language}>
                <option value="zh-CN">简体中文</option>
                <option value="en">English</option>
              </select>
            </div>
          </div>
          <div class="frow">
            <span class="flabel">{t('settings.timezone')}</span>
            <div class="fvalue">
              <input list="tz-list" bind:value={draft.timezone} placeholder="Asia/Shanghai" spellcheck="false" />
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
              </div>
              <button type="button" disabled={taskBusy} onclick={() => void toggleRefreshTask()}>
                {taskStatus.refresh_task ? t('system.refreshTask.uninstall') : t('system.refreshTask.install')}
              </button>
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
          {#if filterOptions}
            <div class="export-filters">
              <label class="efilter">
                {t('export.userFilter')}
                <select bind:value={userFilter}>
                  {#each filterOptions.users as u (u.user_id)}
                    <option value={u.user_id}>{u.name || u.user_id}{u.is_current ? t('export.currentTag') : ''}</option>
                  {/each}
                </select>
              </label>
              <label class="efilter">
                {t('export.hostFilter')}
                <select bind:value={hostFilter}>
                  {#each filterOptions.hosts as h (h.host_id)}
                    <option value={h.host_id}>{h.name || h.host_id}{h.is_current ? t('export.currentTag') : ''}</option>
                  {/each}
                </select>
              </label>
            </div>
          {/if}
          <div class="export">
            <button type="button" onclick={() => void exportData('summary-csv')}>{t('export.csv')}</button>
            <button type="button" onclick={() => void exportData('exchange')}>{t('export.exchange')}</button>
            <button type="button" disabled={importing} onclick={() => void importData()}>
              {importing ? t('common.loading') : t('import.button')}
            </button>
          </div>
          {#if exportMessage}<p class="ok">{exportMessage}</p>{/if}
          {#if exportError}<p class="bad">{exportError}</p>{/if}
          {#if importMessage}<p class="ok">{importMessage}</p>{/if}
          {#if importError}<p class="bad">{importError}</p>{/if}
        </section>
      {:else if sub === 'logs'}
        <section class="panel">
          <div class="logs-toolbar">
            <h4>{t('logs.title')}</h4>
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
          {#if logsLoading && logs.length === 0}
            <p class="hint">{t('common.loading')}</p>
          {:else if logs.length === 0 && !logsError}
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
                  {#each logs as row, i (i)}
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

      {#if sub === 'general' || sub === 'retention' || sub === 'identity'}
        <div class="save-row">
          <button type="submit" class="primary" disabled={saving}>{t('settings.save')}</button>
          {#if message}<span class="ok">{message}</span>{/if}
          {#if errorMessage}<span class="bad">{errorMessage}</span>{/if}
        </div>
      {/if}
    </div>
  </div>

  <datalist id="tz-list">
    {#each TIMEZONES as z (z)}<option value="z"></option>{/each}
  </datalist>

  {#if clearAllOpen}
    <div class="overlay">
      <div class="dialog" role="dialog" aria-modal="true" aria-label={t('cleanup.clearAll')}>
        <p class="dialog-text">{t('cleanup.clearAllConfirm')}</p>
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
    padding: 8px 0;
    border-right: 1px solid #e3e5e8;
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
    color: #444;
  }
  .sidebar button:hover {
    background: #f0f2f5;
  }
  .sidebar button.active {
    background: #e8f0fe;
    color: #1a56c4;
    font-weight: 600;
  }
  .content {
    min-width: 0;
    padding: 8px 0;
  }
  .panel {
    padding: 4px 0 10px;
  }
  .panel + .panel {
    border-top: 1px solid #f0f1f3;
    margin-top: 6px;
    padding-top: 10px;
  }
  .panel h4 {
    font-size: 13px;
    margin: 2px 0 8px;
    color: #333;
  }
  /* 紧凑表单行：行距 10px、标签 140px 右对齐。 */
  .frow {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
    min-width: 0;
  }
  .frow.top {
    align-items: flex-start;
  }
  .flabel {
    flex: none;
    width: 140px;
    text-align: right;
    font-size: 12.5px;
    color: #444;
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
    padding: 4px 8px;
    font-size: 12.5px;
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
    color: #888;
    margin: 2px 0 0 150px;
  }
  .hint {
    font-size: 12px;
    color: #888;
  }
  .name {
    font-size: 12.5px;
    color: #333;
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
    background: #f7f8fa;
    border: 1px solid #e6e8eb;
    border-radius: 8px;
    padding: 5px 10px;
    font-size: 12px;
  }
  .chip .k {
    color: #666;
  }
  .chip .v {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #1c1e21;
  }
  .tier {
    display: grid;
    grid-template-columns: 140px 1fr 110px max-content;
    gap: 8px;
    align-items: center;
    padding: 5px 0;
    margin-bottom: 5px;
    border-bottom: 1px solid #f5f6f7;
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
    color: #666;
  }
  .days {
    width: 90px !important;
    text-align: right;
  }
  .warning {
    font-size: 12px;
    color: #8a6d1a;
    margin: 8px 0 0;
  }
  .sysrow {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 8px 0;
    border-bottom: 1px solid #f0f1f3;
  }
  .sysinfo {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sysinfo .name {
    font-size: 13px;
    color: #333;
  }
  .switch {
    width: 16px;
    height: 16px;
    accent-color: #1a56c4;
    cursor: pointer;
  }
  .meta {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 12px;
    font-size: 12.5px;
    background: #f7f8fa;
    border-radius: 8px;
    padding: 10px 14px;
    align-items: center;
  }
  .meta dt {
    color: #666;
  }
  .mono {
    font-family: ui-monospace, monospace;
    font-size: 12px;
    overflow-wrap: anywhere;
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
  .logs-auto {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: #444;
    cursor: pointer;
  }
  .logs-table-wrap {
    max-height: 440px;
    overflow: auto;
    border: 1px solid #eceef1;
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
    border-bottom: 1px solid #f2f3f5;
    vertical-align: top;
  }
  .logs-table th {
    position: sticky;
    top: 0;
    background: #f7f8fa;
    color: #666;
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
  /* 导出过滤下拉（任务 G）：用户/主机并排。 */
  .export-filters {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }
  .efilter {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: #444;
  }
  .efilter select {
    min-width: 180px;
  }
  button {
    padding: 5px 14px;
    cursor: pointer;
    font-size: 12.5px;
  }
  button.primary {
    background: #1a56c4;
    color: #fff;
    border: none;
    border-radius: 6px;
  }
  button.primary:disabled {
    background: #9db8e8;
    cursor: wait;
  }
  button.danger {
    color: #b3261e;
    border: 1px solid #e3b4b0;
    background: #fff;
    border-radius: 6px;
  }
  button.danger:disabled {
    cursor: wait;
    opacity: 0.6;
  }
  /* 危险操作的实心红样式（清理全部数据）。 */
  button.danger.solid {
    background: #b3261e;
    color: #fff;
    border-color: #b3261e;
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
    background: #fff;
    border-radius: 10px;
    padding: 18px 20px;
    max-width: 480px;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.18);
  }
  .dialog-text {
    font-size: 13px;
    color: #333;
    margin: 0;
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
    color: #1e7a3c;
    font-size: 12.5px;
  }
  .bad {
    color: #b3261e;
    font-size: 12.5px;
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
      border-bottom: 1px solid #e3e5e8;
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
