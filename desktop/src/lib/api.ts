/**
 * Backend IPC types and wrappers. Large token integers use decimal strings; convert to
 * number only for chart scaling, retaining strings for exact values.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** Tiered archive retention in days; yearly null means lifetime retention. */
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
  /** null follows locale: Monday for zh, Sunday for en-US/CA. */
  week_start: number | null;
  retention: RetentionTiers;
  refresh_interval_secs: number;
  pause_on_battery_saver?: boolean;
  close_to_tray?: boolean;
  file_watch_enabled?: boolean;
  language: string;
  /** Theme: system, light or dark. */
  theme: string;
  manual_roots: string[];
  manual_roots_only?: boolean;
  /** Display alias for this source host; does not change host_id. */
  hostname_alias: string | null;
  /** F2 cost estimation, off by default and when absent from historical settings JSON. */
  pricing?: PricingSettings;
  budget?: BudgetSettings;
  otel_receiver_enabled?: boolean;
  otel_receiver_port?: number;
}

export interface BudgetSettings {
  enabled: boolean;
  metric: 'total_tokens' | 'estimated_cost';
  period: 'day' | 'month';
  threshold: string;
  currency: string;
}

export interface BudgetStatusDto {
  enabled: boolean;
  current: string | null;
  threshold: string;
  metric: BudgetSettings['metric'];
  currency: string;
  first_day: string;
  last_day: string;
  coverage_limited: boolean;
  exceeded: boolean;
  newly_triggered: boolean;
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
  verification?: 'waiting' | 'verified' | 'unrecognized' | 'unavailable' | 'external';
  verified_records?: number;
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

/** Provider pricing preferences; absent preferences allow only an unambiguous official API reference for the same model. */
export interface ProviderPricingDefault {
  provider_id: string;
  region: string;
  channel: string;
  /** Default cache-write TTL tier in minutes; null leaves that component unpriced. */
  cache_ttl_minutes: number | null;
}

export interface PricingSettings {
  enabled: boolean;
  provider_defaults: ProviderPricingDefault[];
  /** F2 models.dev catalog refresh, off by default and when absent from historical settings JSON. */
  online_refresh_enabled?: boolean;
  /** Online cache TTL: 1-365 days, default 3; fresh caches avoid network requests. */
  online_cache_ttl_days?: number;
}

/** Amount rows by currency in smallest units; keep currencies separate. */
export interface CostCurrencyRowDto {
  substitute_models?: string[];
  upper_amount_minor?: number | null;
  aggregate_event_count?: number;
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
  /** Events priced through official-provider fallback when the recorded provider lacks an exact entry. */
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
  models: {provider: string; model: string; at_time: CostCurrencyRowDto[]; current_sim: CostCurrencyRowDto[]; unit_prices: UnitPriceDto[]; unpriced_reasons?: Record<string,number>; reference_models?: string[]}[];
  daily: {day: string; provider:string; model:string; sums: CostCurrencyRowDto}[];
  daily_current: {day: string; provider:string; model:string; sums: CostCurrencyRowDto}[];
}

export interface UnitPriceDto {
  price_id: string;
  snapshot_id: string;
  provider_id: string;
  model: string;
  currency: string;
  region: string;
  channel: string;
  service_tier: string;
  context_threshold_tokens: number;
  input_per_mtok_hundredths: number | null;
  output_per_mtok_hundredths: number | null;
  cache_read_per_mtok_hundredths: number | null;
  cache_write_5m_per_mtok_hundredths: number | null;
  cache_write_1h_per_mtok_hundredths: number | null;
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

/** Raw online-response cache metadata for displaying freshness. */
export interface PriceCacheInfoDto {
  fetched_at_ms: number;
  bytes: number;
  content_hash: string;
  age_secs: number;
}

/** Refresh result: fetched, cache_fresh, fetch_failed_used_cache or fetch_failed_no_cache. */
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

/** Intended/actual system-task state; Windows checks due sources each minute. */
export interface SystemTaskStatusDto {
  platform: string;
  auto_start: boolean;
  refresh_task: boolean;
  refresh_task_desired?: boolean;
  refresh_task_exists?: boolean;
  refresh_task_error?: string | null;
  refresh_task_interval?: string;
  unsupported?: boolean;
}

export interface SummaryQuery {
  first_period?: string;
  last_period?: string;
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
  /** null means presence could not be checked; do not hide a real read failure. */
  available?: boolean | null;
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
  /** Registered files no longer on disk after Agent cleanup/compaction; only retained application history remains. */
  missing_files: number;
  /** Source collection schedule; null inherits the global interval (M6). */
  schedule?: {
    kind: 'interval' | 'daily' | 'weekly';
    intervalSeconds: number | null;
    timeOfDay: string | null;
    weekday: number | null;
    nextDueMs?: number | null;
    timezone?: string;
    previewMs?: number[];
  } | null;
}

export interface RefreshStateDto {
  running: boolean;
  started_ms: number;
  last_finished_ms: number;
  trigger: string;
  /** Collection progress: 0-100 percent and estimated remaining seconds; null ETA means unavailable. */
  progress_percent: number;
  eta_seconds: number | null;
  /** Completed adapter names in completion order. */
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
  scheduler_wakeups?: number;
  schema_version: number;
  data_revision: number;
  db_path: string;
  host_id: string;
  exchange_format_version: string;
}

/** v6 user list and currently selected statistics user. */
export interface UserDto {
  user_id: string;
  name: string;
  created_at_ms: number;
}

export interface UsersDto {
  users: UserDto[];
  current: string;
}

/** Row counts by retention layer and database bytes, including WAL. */
export interface StorageStatsDto {
  events: number;
  hourly: number;
  daily: number;
  period: number;
  diagnostics: number;
  db_bytes: number;
  wal_bytes: number;
}

/** Aggregate exchange-package import counts (M1a). */
export interface ImportOutcomeDto {
  details_added?: number;
  details_updated?: number;
  details_unchanged?: number;
  details_skipped?: number;
  details_conflicts?: number;
  cumulative_changed?: number;
  sources_registered: number;
  daily_inserted: number;
  daily_replaced: number;
  daily_skipped: number;
  daily_conflicts: number;
  hourly_inserted: number;
  hourly_replaced: number;
  hourly_skipped: number;
}

/** Manual tiered-cleanup result. */
export interface CleanupResultDto {
  deleted_events: number;
  deleted_hourly_rows: number;
  deleted_daily_rows: number;
  deleted_period_rows: number;
  materialized_period_rows: number;
}

/** Background clear-all start; started=false means a task is already running. */
export interface ClearAllStartDto {
  started: boolean;
}

/** Background clear-all-progress stages.
 * waiting: wait for collection; backup: copy database; clearing: clear transaction;
 * cleared: tables cleared with counts/backup path; rescan: full collection started;
 * done: rescan finished; failed: stopped with error details; cancelled: cancellation acknowledged. */
export interface ClearAllProgressDto {
  phase: 'waiting' | 'backup' | 'clearing' | 'cleared' | 'rescan' | 'done' | 'failed' | 'cancelled';
  cleared?: Record<string, number>;
  data_revision?: number;
  backup?: string | null;
  rescan_started?: boolean;
  error?: string;
}

/** Clear-all preview: event count and registered missing source files that cannot be recollected. */
export interface ClearAllPreviewDto {
  event_count: number;
  missing_files: number;
  total_files: number;
}

/** event_details page row; token integers use decimal strings. */
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

/** Export user/host options and current values; is_current selects defaults. */
export interface ExportFilterOptionsDto {
  users: { user_id: string; name: string; is_current: boolean }[];
  hosts: { host_id: string; name: string | null; is_current: boolean }[];
  current_user: string;
  current_host: string;
}

/** chart_series grouping dimension, queried directly from aggregate tables. */
export type ChartDimension = 'total' | 'model' | 'agent' | 'agent_model';

/** Grouped time-series row; tokens are decimal strings or null for unknown. */
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

/** diagnostic_logs row; time is a millisecond timestamp. */
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

/** Generic quota snapshot; retain native units separately from recorded usage. */
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
    timezone?: string;
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
    kind: 'summary-csv' | 'exchange' | 'details',
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
  /** Diagnostic logs: null code_filter selects all; a code string selects that category. */
  diagnosticLogs: (limit: number, codeFilter: string | null = null) =>
    invoke<DiagnosticLogsDto>('diagnostic_logs', { limit, codeFilter }),
  exportFilterOptions: () => invoke<ExportFilterOptionsDto>('export_filter_options'),
  /** Current API reference, model unit rates and curve; frozen history remains in the response. */
  costSummary: (q: SummaryQuery) => invoke<CostSummaryDto>('cost_summary', { q }),
  recomputeCosts: () => invoke<{ started: boolean }>('recompute_costs'),
  listPriceSnapshots: () => invoke<PriceSnapshotInfoDto[]>('list_price_snapshots'),
  importPriceSnapshot: (path: string) =>
    invoke<{ snapshot_id: string; inserted_rows: number; already_present: boolean }>(
      'import_price_snapshot',
      { path },
    ),
  /** Background models.dev refresh/import; force=true bypasses cache TTL. */
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
  previewExchange: (path: string) => invoke<{ sources: number; details: number; cumulative: number; daily: number; timezone: string }>('preview_exchange', { path }),
  budgetStatus: (claim = false, expectedUser: string | null = null) => invoke<BudgetStatusDto>('budget_status', { claim, expectedUser }),
  storageStats: () => invoke<StorageStatsDto>('storage_stats'),
  manualCleanup: (daysBefore: number) => invoke<CleanupResultDto>('manual_cleanup', { daysBefore }),
  cancelCleanup: () => invoke<boolean>('cancel_cleanup'),
  clearAllData: () => invoke<ClearAllStartDto>('clear_all_data'),
  /** Subscribe to background clear-all stages and return the unsubscribe callback. */
  onClearAllProgress: (handler: (p: ClearAllProgressDto) => void): Promise<UnlistenFn> =>
    listen<ClearAllProgressDto>('clear-all-progress', (e) => handler(e.payload)),
  clearAllPreview: () => invoke<ClearAllPreviewDto>('clear_all_preview'),
  pickOpenPath: (extension: string) => invoke<string | null>('pick_open_path', { extension }),
  /** Generic quota overview; agent=null selects all, with native units separate from recorded usage. */
  quotaSummary: (agent: string | null = null) =>
    invoke<QuotaSummaryDto>('quota_summary', { agent }),
  /** Daily quota trend for (agent, quota_id). */
  quotaSeries: (agent: string, quotaId: string) =>
    invoke<QuotaSeriesDto>('quota_series', { agent, quotaId }),
};

/** Parse a backend JSON-string error containing code/message. */
export function parseError(e: unknown): string {
  if (typeof e === 'string') {
    try {
      const parsed = JSON.parse(e) as { code?: string; message?: string };
      if (parsed.message) return `${parsed.code ?? 'error'}: ${parsed.message}`;
    } catch {
      /* Return the original string when JSON parsing fails. */
    }
    return e;
  }
  return String(e);
}
