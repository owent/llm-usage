//! llm-usage-core: M1 statistics core and SQLite storage.
//!
//! Module responsibilities follow docs/design/desktop-usage/architecture.md:
//! domain (field meanings), metrics (statistics), adapters (source mappings),
//! calendar (IANA calendar), storage (migration/persistence), identity (IDs/deduplication),
//! ingest (atomic batches/jobs), aggregates (native intervals/quotas),
//! query (daily/weekly/monthly queries), retention (retention/sealing),
//! exchange (M1a source identity exchange), pricing (F2 snapshots/estimates).

pub mod adapters;
pub mod aggregates;
pub mod budgets;
pub mod calendar;
mod copilot_carriers;
pub mod copilot_quota;
pub mod detail_exchange;
pub mod domain;
pub mod error;
pub mod exchange;
pub mod exchange_import;
pub mod identity;
pub mod ingest;
pub mod jobs;
pub mod metrics;
pub mod model_names;
pub mod models_dev;
pub mod pricing;
pub mod query;
mod query_acceleration;
pub mod quota_history;
mod qwen_carriers;
pub mod retention;
pub mod retention_tiered;
pub mod schedules;
pub mod storage;
mod summary_cache;

pub use error::CoreError;
pub mod cancellation;
