/**
 * 后端 IPC 类型与封装。token 大数值按合同以十进制字符串传输（number 转换仅用于
 * 图表缩放展示；精确值保留字符串）。
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** 分级归档保留（天；yearly 为 null = 终身）。 */
export interface RetentionTiers {
  events_days: number;
  hourly_days: number;
  daily_days: number;
  weekly_days: number;
  monthly_days: number;
  yearly_days: number | null;
}

export interface AppSettings {
  timezone: string;
  /** null = 跟随语言地区（zh→周一；en-US/CA→周日）。 */
  week_start: number | null;
  retention: RetentionTiers;
  refresh_interval_secs: number;
  language: string;
  /** 主题：system（跟随系统）/ light / dark。 */
  theme: string;
  manual_roots: string[];
  /** 本机来源身份显示名（仅辨认用途，不改 host_id 键）。 */
  hostname_alias: string | null;
  /** F2 费用估算（默认关闭；旧设置 JSON 无此字段时视为关闭）。 */
  pricing?: PricingSettings;
  otel_receiver_enabled?: boolean;
  otel_receiver_port?: number;
}

export interface TelemetryTargetDto {
  id: string;
  name: string;
  config_path: string;
  output_path: string;
  status: 'missing' | 'configured' | 'blocked';
  reason: string;
  configurable: boolean;
  kind: 'jsonc' | 'toml' | 'launcher';
  docs_url: string;
}

export interface TelemetryPreviewDto {
  token: string;
  target: TelemetryTargetDto;
  keys: string[];
  changes: [string, unknown][];
  receiver: boolean;
  sync_config_path?: string | null;
  sync_changes?: [string, unknown][];
}

/** 供应商级估算默认（渠道不明不套价）。 */
export interface ProviderPricingDefault {
  provider_id: string;
  region: string;
  channel: string;
  /** 缓存写默认 TTL 档（分钟；null = 未设，写分量不计价）。 */
  cache_ttl_minutes: number | null;
}

export interface PricingSettings {
  enabled: boolean;
  provider_defaults: ProviderPricingDefault[];
  /** F2 在线刷新（models.dev 社区目录；默认关闭；旧设置 JSON 无此字段时视为关闭）。 */
  online_refresh_enabled?: boolean;
  /** 在线刷新缓存 TTL（天，1–365，默认 7；缓存新鲜期内不发网络请求）。 */
  online_cache_ttl_days?: number;
}

/** 按币种分列的金额行（最小货币单位；不同币种不合并）。 */
export interface CostCurrencyRowDto {
  currency: string;
  total_amount_minor: number;
  input_amount_minor: number | null;
  cache_read_amount_minor: number | null;
  cache_write_amount_minor: number | null;
  output_amount_minor: number | null;
  priced_tokens: number;
  known_tokens: number;
  priced_event_count: number;
  unpriced_event_count: number;
  partial_event_count: number;
  ttl_defaulted_events: number;
  /** 经官方提供商回退定价的事件数（provider 无精确价目时参考模型官方方按量价）。 */
  fallback_event_count: number;
}

export interface CostModeSummaryDto {
  rows: CostCurrencyRowDto[];
  unpriced_reasons: Record<string, number>;
  as_of_ms: number;
  detail_limited: boolean;
}

export interface CostSummaryDto {
  at_time: CostModeSummaryDto;
  source_amounts: CostCurrencyRowDto[];
  current_sim: CostModeSummaryDto;
  price_basis: string[];
  data_revision: number;
}

export interface PriceSnapshotInfoDto {
  snapshot_id: string;
  source_type: string;
  source_urls: string[];
  fetched_at_ms: number;
  verified_at_ms: number | null;
  license: string | null;
  verified_by: string | null;
  note: string | null;
  row_count: number;
}

/** 在线刷新原始响应缓存信息（展示新鲜度）。 */
export interface PriceCacheInfoDto {
  fetched_at_ms: number;
  bytes: number;
  content_hash: string;
  age_secs: number;
}

/** 一次在线刷新的结果。status: fetched / cache_fresh / fetch_failed_used_cache / fetch_failed_no_cache。 */
export interface PriceRefreshOutcomeDto {
  status: string;
  snapshot_id: string | null;
  inserted_rows: number;
  already_present: boolean;
  cache: PriceCacheInfoDto | null;
  error: string | null;
}

export interface PriceRefreshStatusDto {
  enabled: boolean;
  ttl_days: number;
  running: boolean;
  cache: PriceCacheInfoDto | null;
  last_outcome: PriceRefreshOutcomeDto | null;
}

/** 系统任务状态（Windows：开机自启 + 每小时 headless 刷新任务）。 */
export interface SystemTaskStatusDto {
  platform: string;
  auto_start: boolean;
  refresh_task: boolean;
  refresh_task_interval?: string;
  unsupported?: boolean;
}

export interface SummaryQuery {
  first_day: string;
  last_day: string;
  granularity: 'hour' | 'day' | 'week' | 'month';
  agents: string[];
  providers: string[];
  models: string[];
}

export interface MetricSumsDto {
  input_total_known: string | null;
  uncached_known: string | null;
  cache_read_known: string | null;
  cache_write_known: string | null;
  output_total_known: string | null;
  total_tokens_known: string | null;
  input_known_count: number;
  input_unknown_count: number;
  output_known_count: number;
  output_unknown_count: number;
  total_known_count: number;
  total_unknown_count: number;
  event_count: number;
  call_count: number;
  attempt_count: number;
  conflict_count: number;
  cache_input_ratio: number | null;
  avg_duration_ms: string | null;
  total_duration_ms: string | null;
  duration_sample_count: number;
}

export interface PeriodDto {
  label: string;
  start_day: string;
  end_day: string;
  in_progress: boolean;
  partial_history: boolean;
  sums: MetricSumsDto;
  distinct_sessions: number | null;
  active_days: number | null;
}

export interface SummaryDto {
  data_revision: number;
  timezone: string;
  periods: PeriodDto[];
  totals: MetricSumsDto;
  distinct_sessions: number | null;
  active_days: number | null;
  models: { provider: string | null; model: string | null; sums: MetricSumsDto }[];
  agents: { agent: string; sums: MetricSumsDto }[];
  today_hourly: {
    hour: number;
    calls: number;
    total_tokens: string | null;
    input_total: string | null;
    cache_read: string | null;
    output_total: string | null;
    sessions: number | null;
    avg_duration_ms: string | null;
  }[];
  excluded_event_count: number;
}

export interface HeatmapDto {
  cells: { day: string; weekday: number; calls: number; total_tokens: string | null; available: boolean; partial: boolean }[];
}

export interface SourceDto {
  instance_id: string;
  agent: string;
  format: string | null;
  health: string;
  enabled: boolean;
  origin_host_id: string;
  user_id: string;
  last_success_ms: number | null;
  compat_files: number;
  degraded_files: number;
  unsupported_files: number;
  incompatible_files: number;
  /** 注册过但磁盘已不存在（Agent 自行清理/压实）：其历史只存于本应用存档。 */
  missing_files: number;
  /** 逐源提取计划（null = 继承全局间隔；M6 逐源定时）。 */
  schedule?: {
    kind: 'interval' | 'daily' | 'weekly';
    intervalSeconds: number | null;
    timeOfDay: string | null;
    weekday: number | null;
    nextDueMs?: number | null;
  } | null;
}

export interface RefreshStateDto {
  running: boolean;
  started_ms: number;
  last_finished_ms: number;
  trigger: string;
  /** 采集进度（0–100 百分比 + 预计剩余秒；null = 暂不可估）。 */
  progress_percent: number;
  eta_seconds: number | null;
  /** 已完成的适配器名（按完成顺序）。 */
  completed_adapters: string[];
  instances: {
    instance_id: string;
    agent: string;
    status: string;
    error: string | null;
    added: number;
    updated: number;
    files: number;
    events: number;
    diagnostics: number;
  }[];
}

export interface AppInfoDto {
  schema_version: number;
  data_revision: number;
  db_path: string;
  host_id: string;
  exchange_format_version: string;
}

/** 多用户（v6）：用户清单 + 当前统计用户。 */
export interface UserDto {
  user_id: string;
  name: string;
  created_at_ms: number;
}

export interface UsersDto {
  users: UserDto[];
  current: string;
}

/** 各归档层条目数 + 库文件占用（含 WAL）。 */
export interface StorageStatsDto {
  events: number;
  hourly: number;
  daily: number;
  period: number;
  diagnostics: number;
  db_bytes: number;
  wal_bytes: number;
}

/** 聚合交换包导入计数（M1a 合同）。 */
export interface ImportOutcomeDto {
  sources_registered: number;
  daily_inserted: number;
  daily_replaced: number;
  daily_skipped: number;
  daily_conflicts: number;
  hourly_inserted: number;
  hourly_replaced: number;
  hourly_skipped: number;
}

/** 手动分层清理结果。 */
export interface CleanupResultDto {
  deleted_events: number;
  deleted_hourly_rows: number;
  deleted_daily_rows: number;
  deleted_period_rows: number;
  materialized_period_rows: number;
}

/** 清理全部数据：后台任务启动结果（started=false 表示已有任务在执行）。 */
export interface ClearAllStartDto {
  started: boolean;
}

/** 清理全部数据后台任务阶段事件（clear-all-progress）。
 * waiting=等当前采集结束；backup=备份数据库；clearing=清库事务；
 * cleared=清库完成（含各表计数/备份路径）；rescan=全量重采已开始；
 * done=重采结束；failed=失败终止（error 含原因）。 */
export interface ClearAllProgressDto {
  phase: 'waiting' | 'backup' | 'clearing' | 'cleared' | 'rescan' | 'done' | 'failed';
  cleared?: Record<string, number>;
  data_revision?: number;
  backup?: string | null;
  rescan_started?: boolean;
  error?: string;
}

/** 清空预检：事件总量 + 已注册但磁盘不存在的源文件数（这些历史无法重采）。 */
export interface ClearAllPreviewDto {
  event_count: number;
  missing_files: number;
  total_files: number;
}

/** 用量明细分页行（event_details 命令）。token 为十进制字符串。 */
export interface EventDetailRowDto {
  event_id: string;
  agent: string;
  model: string | null;
  category: string | null;
  occurred_at_ms: number;
  session: string | null;
  input: string | null;
  cache_read: string | null;
  output: string | null;
  total: string | null;
  duration_ms: string | null;
  lifecycle: string;
}

export interface EventDetailsDto {
  rows: EventDetailRowDto[];
  total: number;
  page: number;
  page_size: number;
}

/** 导出过滤选项：可用用户/主机 + 当前值（is_current 标记默认选中项）。 */
export interface ExportFilterOptionsDto {
  users: { user_id: string; name: string; is_current: boolean }[];
  hosts: { host_id: string; name: string | null; is_current: boolean }[];
  current_user: string;
  current_host: string;
}

/** chart_series 维度分组模式（图表分组数据源；从聚合表直读）。 */
export type ChartDimension = 'total' | 'model' | 'agent' | 'agent_model';

/** 维度分组时间序列行（token 为十进制字符串或 null=未知）。 */
export interface ChartSeriesRowDto {
  cache_write: string | null;
  uncached: string | null;
  cache_ratio: number | null;
  label: string;
  series: string;
  calls: number;
  input: string | null;
  cache_read: string | null;
  output: string | null;
  total: string | null;
}

export interface ChartSeriesDto {
  rows: ChartSeriesRowDto[];
}

/** 诊断日志行（diagnostic_logs 命令；时间为毫秒时间戳）。 */
export interface DiagnosticLogRowDto {
  time: number;
  code: string;
  field: string | null;
  instance: string | null;
  message: string;
}

export interface DiagnosticLogsDto {
  rows: DiagnosticLogRowDto[];
}

/** 通用额度快照（请求/额度计数，非 token）。 */
export interface QuotaDto {
  agent: string;
  quota_id: string;
  kind: string;
  unit: string;
  limit_value: number | null;
  used: number | null;
  remaining: number | null;
  percent_remaining: number | null;
  locality_verified: boolean;
  observed_at_ms: number;
}
export interface QuotaSummaryDto {
  quotas: QuotaDto[];
}
export interface QuotaPointDto {
  local_day: string;
  used: number | null;
  remaining: number | null;
  limit_value: number | null;
}
export interface QuotaSeriesDto {
  agent: string;
  quota_id: string;
  points: QuotaPointDto[];
}

export const api = {
  telemetryCheck: () => invoke<TelemetryTargetDto[]>('telemetry_check'),
  telemetryPreview: (id: string) => invoke<TelemetryPreviewDto>('telemetry_preview', { id }),
  telemetryApply: (token: string) => invoke<void>('telemetry_apply', { token }),
  telemetryUndo: (token: string) => invoke<string[]>('telemetry_undo', { token }),
  summary: (q: SummaryQuery) => invoke<SummaryDto>('summary', { q }),
  heatmap: (q: SummaryQuery) => invoke<HeatmapDto>('heatmap', { q }),
  listSources: () => invoke<{ sources: SourceDto[] }>('list_sources'),
  setSourceSchedule: (instanceId: string, rule: {
    kind: 'interval' | 'daily' | 'weekly';
    intervalSeconds?: number | null;
    timeOfDay?: string | null;
    weekday?: number | null;
  } | null) =>
    invoke<unknown>('set_source_schedule', { instanceId, rule }),
  setSourceEnabled: (instanceId: string, enabled: boolean) =>
    invoke<void>('set_source_enabled', { instanceId, enabled }),
  refreshSources: () =>
    invoke<{ started: boolean; running: boolean; last_finished_ms: number }>('refresh_sources'),
  refreshStatus: () => invoke<RefreshStateDto>('refresh_status'),
  getSettings: () => invoke<AppSettings>('get_settings'),
  setSettings: (settings: AppSettings) => invoke<void>('set_settings', { settings }),
  appInfo: () => invoke<AppInfoDto>('app_info'),
  exportData: (
    kind: 'summary-csv' | 'exchange',
    targetDir: string | null,
    q: SummaryQuery,
    userFilter: string | null = null,
    hostFilter: string | null = null,
  ) =>
    invoke<{ path: string; kind: string }>('export_data', {
      kind,
      targetDir,
      q,
      userFilter,
      hostFilter,
    }),
  eventDetails: (q: SummaryQuery, page: number, pageSize: number) =>
    invoke<EventDetailsDto>('event_details', { q, page, pageSize }),
  chartSeries: (q: SummaryQuery, dimension: ChartDimension) =>
    invoke<ChartSeriesDto>('chart_series', { q, dimension }),
  /** 诊断日志（code_filter 为 null = 全部；传 code 字符串只取该类）。 */
  diagnosticLogs: (limit: number, codeFilter: string | null = null) =>
    invoke<DiagnosticLogsDto>('diagnostic_logs', { limit, codeFilter }),
  exportFilterOptions: () => invoke<ExportFilterOptionsDto>('export_filter_options'),
  /** F2 费用汇总（按发生时价 / 来源金额 / 按当前价格模拟分列）。 */
  costSummary: (q: SummaryQuery) => invoke<CostSummaryDto>('cost_summary', { q }),
  recomputeCosts: () => invoke<{ started: boolean }>('recompute_costs'),
  listPriceSnapshots: () => invoke<PriceSnapshotInfoDto[]>('list_price_snapshots'),
  importPriceSnapshot: (path: string) =>
    invoke<{ snapshot_id: string; inserted_rows: number; already_present: boolean }>(
      'import_price_snapshot',
      { path },
    ),
  /** F2 在线刷新：后台抓取 models.dev 并导入（force=true 绕过缓存 TTL）。 */
  refreshPricesOnline: (force: boolean) =>
    invoke<{ started: boolean }>('refresh_prices_online', { force }),
  priceRefreshStatus: () => invoke<PriceRefreshStatusDto>('price_refresh_status'),
  pickSavePath: (defaultName: string) =>
    invoke<string | null>('pick_save_path', { defaultName }),
  systemTaskStatus: () => invoke<SystemTaskStatusDto>('system_task_status'),
  setAutoStart: (enabled: boolean) => invoke<void>('set_auto_start', { enabled }),
  setRefreshTask: (install: boolean) => invoke<void>('set_refresh_task', { install }),
  listUsers: () => invoke<UsersDto>('list_users'),
  createUser: (name: string, switchTo: boolean) =>
    invoke<{ user_id: string }>('create_user', { name, switch: switchTo }),
  setCurrentUser: (userId: string) => invoke<void>('set_current_user', { userId }),
  assignSourceUser: (instanceId: string, userId: string) =>
    invoke<void>('assign_source_user', { instanceId, userId }),
  importExchange: (path: string) => invoke<ImportOutcomeDto>('import_exchange', { path }),
  storageStats: () => invoke<StorageStatsDto>('storage_stats'),
  manualCleanup: (daysBefore: number) => invoke<CleanupResultDto>('manual_cleanup', { daysBefore }),
  clearAllData: () => invoke<ClearAllStartDto>('clear_all_data'),
  /** 订阅清理全部数据后台任务的阶段进度；返回取消订阅函数。 */
  onClearAllProgress: (handler: (p: ClearAllProgressDto) => void): Promise<UnlistenFn> =>
    listen<ClearAllProgressDto>('clear-all-progress', (e) => handler(e.payload)),
  clearAllPreview: () => invoke<ClearAllPreviewDto>('clear_all_preview'),
  pickOpenPath: (extension: string) => invoke<string | null>('pick_open_path', { extension }),
  /** 通用额度总览（agent=null 取全部；请求/额度计数，非 token）。 */
  quotaSummary: (agent: string | null = null) =>
    invoke<QuotaSummaryDto>('quota_summary', { agent }),
  /** 某 (agent, quota_id) 的每日额度趋势。 */
  quotaSeries: (agent: string, quotaId: string) =>
    invoke<QuotaSeriesDto>('quota_series', { agent, quotaId }),
};

/** 结构化错误解析（后端返回 JSON 字符串 code+message）。 */
export function parseError(e: unknown): string {
  if (typeof e === 'string') {
    try {
      const parsed = JSON.parse(e) as { code?: string; message?: string };
      if (parsed.message) return `${parsed.code ?? 'error'}: ${parsed.message}`;
    } catch {
      /* 原样返回 */
    }
    return e;
  }
  return String(e);
}
