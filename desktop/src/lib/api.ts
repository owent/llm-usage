/**
 * 后端 IPC 类型与封装。token 大数值按合同以十进制字符串传输（number 转换仅用于
 * 图表缩放展示；精确值保留字符串）。
 */
import { invoke } from '@tauri-apps/api/core';

export interface AppSettings {
  timezone: string;
  week_start: number;
  retention_days: number | null;
  refresh_interval_secs: number;
  language: string;
  manual_roots: string[];
}

export interface SummaryQuery {
  first_day: string;
  last_day: string;
  granularity: 'day' | 'week' | 'month';
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
  models: { provider: string | null; model: string | null; sums: MetricSumsDto }[];
  agents: { agent: string; sums: MetricSumsDto }[];
  today_hourly: {
    hour: number;
    calls: number;
    total_tokens: string | null;
    input_total: string | null;
    output_total: string | null;
  }[];
  excluded_event_count: number;
}

export interface HeatmapDto {
  cells: { weekday: number; hour: number; calls: number; total_tokens: string | null }[];
}

export interface SourceDto {
  instance_id: string;
  agent: string;
  format: string | null;
  health: string;
  enabled: boolean;
  origin_host_id: string;
  last_success_ms: number | null;
  compat_files: number;
  incompatible_files: number;
}

export interface RefreshStateDto {
  running: boolean;
  started_ms: number;
  last_finished_ms: number;
  trigger: string;
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

export const api = {
  summary: (q: SummaryQuery) => invoke<SummaryDto>('summary', { q }),
  heatmap: (q: SummaryQuery) => invoke<HeatmapDto>('heatmap', { q }),
  listSources: () => invoke<{ sources: SourceDto[] }>('list_sources'),
  setSourceEnabled: (instanceId: string, enabled: boolean) =>
    invoke<void>('set_source_enabled', { instanceId, enabled }),
  refreshSources: () =>
    invoke<{ started: boolean; running: boolean; last_finished_ms: number }>('refresh_sources'),
  refreshStatus: () => invoke<RefreshStateDto>('refresh_status'),
  getSettings: () => invoke<AppSettings>('get_settings'),
  setSettings: (settings: AppSettings) => invoke<void>('set_settings', { settings }),
  appInfo: () => invoke<AppInfoDto>('app_info'),
  exportData: (kind: 'summary-csv' | 'exchange', targetDir: string | null, q: SummaryQuery) =>
    invoke<{ path: string; kind: string }>('export_data', { kind, targetDir, q }),
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
