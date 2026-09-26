//! SQLite schema（预发布阶段：不做逐版本迁移，只建当前 schema）。
//!
//! 合同（2026-09-26 用户决策）：
//! - 未发布过，不记录每个版本的数据库迁移历史；
//! - 打开时发现 user_version != SCHEMA_VERSION ⇒ 返回 SchemaTooNew/SchemaTooOld；
//! - 由应用层提示用户"数据库版本不兼容，是否全量删除重建"；
//! - 用户允许 ⇒ 删除整个数据库文件重新创建；不允许 ⇒ 退出应用。

/// 本程序支持的最新 schema 版本（唯一有效值）。
pub const SCHEMA_VERSION: u32 = 7;

/// 完整建库 SQL（新库一步到位；不做增量迁移）。
pub const FULL_SCHEMA: &str = r#"
PRAGMA defer_foreign_keys = ON;

CREATE TABLE source_instances (
  instance_id TEXT PRIMARY KEY,
  agent TEXT NOT NULL,
  host_application TEXT,
  locality_basis TEXT NOT NULL,
  attribution_status TEXT NOT NULL,
  exclusion_reason TEXT,
  enabled INTEGER NOT NULL DEFAULT 1,
  format TEXT,
  location_hint TEXT,
  parser_version TEXT,
  capabilities TEXT,
  health TEXT NOT NULL DEFAULT 'ok',
  origin_host_id TEXT NOT NULL DEFAULT 'legacy_unknown',
  user_id TEXT NOT NULL DEFAULT 'default',
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_source_instances_host ON source_instances(origin_host_id);
CREATE INDEX idx_source_instances_user ON source_instances(user_id);

CREATE TABLE origin_hosts (
  host_id TEXT PRIMARY KEY,
  is_local INTEGER NOT NULL DEFAULT 0,
  note TEXT,
  first_seen_ms INTEGER NOT NULL,
  last_seen_ms INTEGER NOT NULL
);

CREATE TABLE origin_host_names (
  host_id TEXT NOT NULL REFERENCES origin_hosts(host_id),
  hostname TEXT NOT NULL,
  first_seen_ms INTEGER NOT NULL,
  last_seen_ms INTEGER NOT NULL,
  PRIMARY KEY (host_id, hostname)
);

CREATE TABLE users (
  user_id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL
);
INSERT INTO users (user_id, name, created_at_ms) VALUES ('default', 'default', 0);

CREATE TABLE source_files (
  file_id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL REFERENCES source_instances(instance_id),
  file_identity TEXT NOT NULL,
  generation INTEGER NOT NULL DEFAULT 0,
  byte_size INTEGER,
  mtime_ms INTEGER,
  content_hash TEXT,
  format_status TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  first_seen_ms INTEGER NOT NULL,
  last_seen_ms INTEGER NOT NULL,
  UNIQUE(instance_id, file_identity)
);

CREATE TABLE ingestion_checkpoints (
  instance_id TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  cursor_value TEXT,
  parse_context TEXT,
  source_revision INTEGER,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (instance_id, scope_key)
);

CREATE TABLE usage_events (
  event_id TEXT PRIMARY KEY,
  source_instance_id TEXT NOT NULL,
  source_record_key TEXT NOT NULL,
  record_kind TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  parser_version TEXT NOT NULL,
  parse_basis TEXT,
  origin_call_id TEXT,
  attempt_id TEXT,
  session_id TEXT,
  parent_session_id TEXT,
  host_application TEXT,
  agent TEXT NOT NULL,
  call_category TEXT NOT NULL DEFAULT 'unknown',
  occurred_at_ms INTEGER NOT NULL,
  observed_at_ms INTEGER,
  source_time TEXT,
  time_basis TEXT NOT NULL,
  interval_start_ms INTEGER,
  interval_end_ms INTEGER,
  provider_id TEXT,
  model_raw TEXT,
  model_canonical TEXT,
  model_attribution TEXT NOT NULL DEFAULT 'unknown',
  input_uncached INTEGER,
  input_cache_read INTEGER,
  input_cache_write INTEGER,
  input_total INTEGER,
  output_total INTEGER,
  output_reasoning INTEGER,
  total_tokens INTEGER,
  source_total INTEGER,
  quality_json TEXT NOT NULL,
  quality_bucket TEXT NOT NULL,
  lifecycle TEXT NOT NULL,
  source_revision INTEGER,
  error_status TEXT,
  duration_ms INTEGER,
  ttft_ms INTEGER,
  attribution_status TEXT NOT NULL DEFAULT 'verified',
  exclusion_reason TEXT,
  conflict INTEGER NOT NULL DEFAULT 0,
  content_hash TEXT NOT NULL,
  cost_amount_minor INTEGER,
  cost_currency TEXT,
  cost_kind TEXT,
  price_version TEXT,
  billing_scope TEXT,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  UNIQUE(source_instance_id, source_record_key)
);
CREATE INDEX idx_usage_events_occurred ON usage_events(occurred_at_ms);
CREATE INDEX idx_usage_events_model_time ON usage_events(provider_id, model_raw, occurred_at_ms);
CREATE INDEX idx_usage_events_agent_time ON usage_events(agent, occurred_at_ms);
CREATE INDEX idx_usage_events_revision ON usage_events(source_instance_id, source_revision);
CREATE INDEX idx_usage_events_instance_time ON usage_events(source_instance_id, occurred_at_ms DESC);

CREATE TABLE event_aliases (
  canonical_event_id TEXT NOT NULL REFERENCES usage_events(event_id),
  member_event_id TEXT NOT NULL REFERENCES usage_events(event_id),
  link_basis TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  PRIMARY KEY (canonical_event_id, member_event_id)
);

CREATE TABLE source_aggregates (
  aggregate_id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL,
  scope TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  interval_start_ms INTEGER,
  interval_end_ms INTEGER NOT NULL,
  interval_end_inclusive INTEGER NOT NULL DEFAULT 0,
  input_uncached INTEGER,
  input_cache_read INTEGER,
  input_cache_write INTEGER,
  input_total INTEGER,
  output_total INTEGER,
  output_reasoning INTEGER,
  total_tokens INTEGER,
  source_total INTEGER,
  quality_json TEXT NOT NULL,
  reported_call_count INTEGER,
  coverage TEXT NOT NULL DEFAULT 'exclusive',
  duplicate_of TEXT,
  time_basis TEXT NOT NULL DEFAULT 'uncertain',
  source_revision INTEGER,
  content_hash TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  UNIQUE(instance_id, scope, scope_key)
);

CREATE TABLE hourly_usage (
  tz_version TEXT NOT NULL,
  local_day TEXT NOT NULL,
  hour INTEGER NOT NULL,
  instance_id TEXT NOT NULL DEFAULT 'legacy_unknown',
  agent TEXT NOT NULL,
  provider_id TEXT NOT NULL DEFAULT '',
  model_raw TEXT NOT NULL DEFAULT '',
  call_category TEXT NOT NULL,
  quality_bucket TEXT NOT NULL,
  event_count INTEGER NOT NULL,
  call_count INTEGER NOT NULL,
  input_known_sum INTEGER,
  cache_read_known_sum INTEGER,
  cache_write_known_sum INTEGER,
  output_known_sum INTEGER,
  total_known_sum INTEGER,
  conflict_count INTEGER NOT NULL,
  data_revision INTEGER NOT NULL,
  PRIMARY KEY (tz_version, local_day, hour, instance_id, agent, provider_id, model_raw, call_category, quality_bucket)
);
CREATE INDEX idx_hourly_usage_day ON hourly_usage(tz_version, local_day);

CREATE TABLE period_usage (
  tz_version TEXT NOT NULL,
  granularity TEXT NOT NULL,
  period_key TEXT NOT NULL,
  period_start_day TEXT NOT NULL,
  period_end_day TEXT NOT NULL,
  instance_id TEXT NOT NULL DEFAULT 'legacy_unknown',
  agent TEXT NOT NULL,
  provider_id TEXT NOT NULL DEFAULT '',
  model_raw TEXT NOT NULL DEFAULT '',
  call_category TEXT NOT NULL,
  quality_bucket TEXT NOT NULL,
  event_count INTEGER NOT NULL,
  call_count INTEGER NOT NULL,
  input_known_sum INTEGER,
  cache_read_known_sum INTEGER,
  cache_write_known_sum INTEGER,
  output_known_sum INTEGER,
  total_known_sum INTEGER,
  conflict_count INTEGER NOT NULL,
  active_days INTEGER NOT NULL,
  distinct_sessions INTEGER,
  materialized_at_ms INTEGER NOT NULL,
  data_revision INTEGER NOT NULL,
  PRIMARY KEY (tz_version, granularity, period_key, instance_id, agent, provider_id, model_raw, call_category, quality_bucket)
);
CREATE INDEX idx_period_usage_range ON period_usage(tz_version, granularity, period_start_day);

CREATE TABLE quota_snapshots (
  quota_id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL,
  observed_at_ms INTEGER NOT NULL,
  kind TEXT NOT NULL,
  quantity_minor INTEGER,
  unit TEXT NOT NULL,
  window_start_ms INTEGER,
  window_end_ms INTEGER,
  locality_verified INTEGER NOT NULL DEFAULT 0,
  detail_json TEXT,
  created_at_ms INTEGER NOT NULL
);

CREATE TABLE daily_usage (
  tz_version TEXT NOT NULL,
  local_day TEXT NOT NULL,
  instance_id TEXT NOT NULL DEFAULT 'legacy_unknown',
  agent TEXT NOT NULL,
  provider_id TEXT NOT NULL DEFAULT '',
  model_raw TEXT NOT NULL DEFAULT '',
  call_category TEXT NOT NULL,
  quality_bucket TEXT NOT NULL,
  event_count INTEGER NOT NULL,
  call_count INTEGER NOT NULL,
  attempt_count INTEGER NOT NULL,
  observation_count INTEGER NOT NULL,
  input_known_sum INTEGER,
  input_known_count INTEGER NOT NULL,
  input_unknown_count INTEGER NOT NULL,
  uncached_known_sum INTEGER,
  uncached_known_count INTEGER NOT NULL,
  cache_read_known_sum INTEGER,
  cache_read_known_count INTEGER NOT NULL,
  cache_write_known_sum INTEGER,
  cache_write_known_count INTEGER NOT NULL,
  output_known_sum INTEGER,
  output_known_count INTEGER NOT NULL,
  output_unknown_count INTEGER NOT NULL,
  total_known_sum INTEGER,
  total_known_count INTEGER NOT NULL,
  total_unknown_count INTEGER NOT NULL,
  ratio_input_sum INTEGER,
  ratio_cache_read_sum INTEGER,
  ratio_sample_count INTEGER NOT NULL,
  conflict_count INTEGER NOT NULL,
  sealed INTEGER NOT NULL DEFAULT 0,
  sealed_at_ms INTEGER,
  seal_tz TEXT,
  seal_field_version TEXT,
  seal_source_version TEXT,
  data_revision INTEGER NOT NULL,
  PRIMARY KEY (tz_version, local_day, instance_id, agent, provider_id, model_raw, call_category, quality_bucket)
);

CREATE TABLE aggregate_generations (
  generation_id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  tz_version TEXT,
  status TEXT NOT NULL,
  note TEXT,
  created_at_ms INTEGER NOT NULL,
  published_at_ms INTEGER
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE model_aliases (
  provider_id TEXT NOT NULL,
  model_raw TEXT NOT NULL,
  rule_version TEXT NOT NULL,
  alias TEXT,
  family TEXT,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (provider_id, model_raw, rule_version)
);

CREATE TABLE price_versions (
  price_id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  model_canonical TEXT NOT NULL,
  region TEXT,
  channel TEXT,
  effective_from_ms INTEGER NOT NULL,
  effective_to_ms INTEGER,
  input_per_mtok_minor INTEGER,
  cache_read_per_mtok_minor INTEGER,
  cache_write_per_mtok_minor INTEGER,
  output_per_mtok_minor INTEGER,
  currency TEXT NOT NULL,
  price_version TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL
);

CREATE TABLE extraction_schedules (
  schedule_id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  instance_id TEXT,
  rule_kind TEXT NOT NULL,
  interval_seconds INTEGER,
  time_of_day TEXT,
  weekday INTEGER,
  tz TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  paused_reason TEXT,
  file_trigger_enabled INTEGER NOT NULL DEFAULT 0,
  config_version INTEGER NOT NULL,
  next_due_at_ms INTEGER,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE schedule_state (
  schedule_id TEXT PRIMARY KEY REFERENCES extraction_schedules(schedule_id),
  desired_state TEXT NOT NULL,
  applied_state TEXT NOT NULL,
  last_started_ms INTEGER,
  last_success_ms INTEGER,
  last_duration_ms INTEGER,
  last_run_stats TEXT,
  error_summary TEXT,
  next_run_ms INTEGER,
  updated_ms INTEGER NOT NULL
);

CREATE TABLE ingest_runs (
  run_id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL,
  trigger_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  merged_triggers TEXT NOT NULL DEFAULT '[]',
  added INTEGER NOT NULL DEFAULT 0,
  updated INTEGER NOT NULL DEFAULT 0,
  unchanged INTEGER NOT NULL DEFAULT 0,
  skipped INTEGER NOT NULL DEFAULT 0,
  errors INTEGER NOT NULL DEFAULT 0,
  error_summary TEXT,
  started_ms INTEGER NOT NULL,
  finished_ms INTEGER,
  data_revision INTEGER
);
CREATE INDEX idx_ingest_runs_source_status ON ingest_runs(instance_id, status);

CREATE TABLE diagnostics (
  diag_id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id TEXT,
  run_id TEXT,
  instance_id TEXT,
  event_id TEXT,
  code TEXT NOT NULL,
  field TEXT,
  position TEXT,
  message TEXT NOT NULL,
  created_ms INTEGER NOT NULL
);
CREATE INDEX idx_diagnostics_created ON diagnostics(created_ms);
CREATE INDEX idx_diagnostics_created_code ON diagnostics(created_ms DESC, code);

CREATE TABLE import_manifests (
  import_id TEXT PRIMARY KEY,
  source_digest TEXT NOT NULL,
  scope TEXT NOT NULL,
  status TEXT NOT NULL,
  stats TEXT,
  rollback_info TEXT,
  created_ms INTEGER NOT NULL,
  finished_ms INTEGER
);

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_ms INTEGER NOT NULL,
  notes TEXT
);
"#;
