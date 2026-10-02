//! llm-usage-core：M1 统计核心与 SQLite 存储。
//!
//! 模块边界遵循 docs/design/desktop-usage/architecture.md：
//! domain（统计语义）、metrics（统计数学）、adapters（来源字段口径映射）、
//! calendar（IANA 时区日历）、storage（迁移与持久化）、identity（身份与去重）、
//! ingest（原子批次与作业）、aggregates（来源原生区间汇总与额度）、
//! query（日/周/月汇总查询）、retention（保留与封存）、
//! exchange（M1a 来源身份交换合同）、pricing（F2 价格快照与费用估算）。

pub mod adapters;
pub mod aggregates;
pub mod calendar;
mod copilot_carriers;
pub mod copilot_quota;
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
pub mod quota_history;
pub mod retention;
pub mod retention_tiered;
pub mod schedules;
pub mod storage;

pub use error::CoreError;
