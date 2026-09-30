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

export const api = {
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
