//! 历史来源交换合同（M1a，data-contract.md#provenance）：
//! 版本化导出格式 + 重复跳过/权威修订替换/互斥来源新增/冲突保留的合并判定。
//!
//! 边界：
//! - 本模块只定义格式与判定；实际导入/Merge 写入另行排期（M5/M6）。
//! - 展示用 CSV/图表不满足本合同，不得被当作无损回导文件。
//! - 批次身份（batch_id）只保证导入操作幂等，不替代来源记录身份。
//! - 导出允许主机名别名化或省略（redact_hostnames），稳定来源键不重写。

use crate::error::CoreError;
use crate::identity::{arbitrate, Arbitration, ExistingMeta};
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const EXCHANGE_FORMAT_VERSION: &str = "llm-usage-exchange-1";

/// 导出类型：完整快照或增量。同一范围未出现于增量包不表示删除；
/// 删除或整段替换语义由导出方在 `deletions` 中显式声明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ExchangeKind {
    FullSnapshot,
    Incremental {
        /// 增量起点（上次导出的批次 ID 或事件时间下界，毫秒）；None 表示未约束。
        since_ms: Option<i64>,
        /// 显式删除/整段替换声明：被删除事件的 (source_instance_id, source_record_key)。
        /// 空表示无删除语义。
        deletions: Vec<DeletedRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedRecord {
    pub source_instance_id: String,
    pub source_record_key: String,
    /// 删除依据（如源端清理观察）；不含正文。
    pub reason: String,
}

/// 来源主机注册信息。主机名可脱敏（别名化/省略），host_id 不重写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeHost {
    pub origin_host_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname_alias: Option<String>,
}

/// 来源实例注册信息（原始来源，导入机不得改成自己）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeSource {
    pub source_instance_id: String,
    pub agent: String,
    pub format: String,
    pub parser_version: String,
    pub locality_basis: String,
    pub attribution_status: String,
    pub first_seen_ms: i64,
    /// 字段完整性/能力摘要（白名单 JSON，无正文）。
    pub completeness: serde_json::Value,
}

/// 逐事件交换记录：来源 + 记录键构成逻辑唯一键；数值与完整性随记录走。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeRecord {
    pub source_instance_id: String,
    pub source_record_key: String,
    pub record_kind: String,
    pub schema_version: String,
    pub parser_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_basis: Option<String>,
    pub occurred_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
    pub time_basis: String,
    pub agent: String,
    pub call_category: String,
    pub lifecycle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// 八个 token 字段可空（unknown 不补零）。
    pub usage: ExchangeUsage,
    /// 逐字段质量（reported/derived/unknown），与 token 字段质量分别记录。
    pub quality_bucket: String,
    pub source_revision: Option<i64>,
    pub conflict: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_note: Option<String>,
}

/// 与 usage_events 数值列一一对应；null = unknown。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_uncached: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_total: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_total: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_reasoning: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_total: Option<i64>,
}

/// 日/封存分区（明细已清理时历史汇总仍可导出，保留来源与修订）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeDailyPartition {
    pub tz_version: String,
    pub local_day: String,
    pub instance_id: String,
    pub agent: String,
    pub provider_id: String,
    pub model_raw: String,
    pub call_category: String,
    pub quality_bucket: String,
    pub event_count: i64,
    pub call_count: i64,
    pub input_known_sum: Option<i64>,
    pub cache_read_known_sum: Option<i64>,
    pub cache_write_known_sum: Option<i64>,
    pub output_known_sum: Option<i64>,
    pub total_known_sum: Option<i64>,
    pub conflict_count: i64,
    pub sealed: bool,
    pub data_revision: i64,
}

/// 小时层交换行（分级归档的 30 天层；导入按 (tz,day,hour,dims) 键合并）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeHourlyPartition {
    pub tz_version: String,
    pub local_day: String,
    pub hour: i64,
    pub instance_id: String,
    pub agent: String,
    pub provider_id: String,
    pub model_raw: String,
    pub call_category: String,
    pub quality_bucket: String,
    pub event_count: i64,
    pub call_count: i64,
    pub input_known_sum: Option<i64>,
    pub cache_read_known_sum: Option<i64>,
    pub cache_write_known_sum: Option<i64>,
    pub output_known_sum: Option<i64>,
    pub total_known_sum: Option<i64>,
    pub data_revision: i64,
}

/// 完整导出包。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeExport {
    pub format_version: String,
    pub kind: ExchangeKind,
    /// 批次身份：同一批次重复导入幂等；不替代记录身份。
    pub batch_id: String,
    pub exported_at_ms: i64,
    pub timezone: String,
    pub host: ExchangeHost,
    pub sources: Vec<ExchangeSource>,
    pub records: Vec<ExchangeRecord>,
    /// 封存/已清理明细的日分区（与 records 互补；同时导出时以 records 明细为准，
    /// 分区仅在对应明细缺失时作为汇总对照）。
    #[serde(default)]
    pub daily_partitions: Vec<ExchangeDailyPartition>,
    /// 小时层（今日小时图在明细删除后的数据来源）。
    #[serde(default)]
    pub hourly_partitions: Vec<ExchangeHourlyPartition>,
}

/// 导出请求。
#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub timezone: String,
    /// 事件时间下界/上界（毫秒，半开区间语义由调用方定义并写入导出）。
    pub from_ms: i64,
    pub to_ms: i64,
    /// 只导出这些来源实例；空 = 全部。
    pub instances: Vec<String>,
    /// true 时省略主机名（导出脱敏）。
    pub redact_hostnames: bool,
    pub kind: ExchangeKind,
    pub batch_id: String,
}

/// 构建导出包（只读查询）。
pub fn build_export(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
) -> Result<ExchangeExport, CoreError> {
    let local_host = storage.local_host_id()?;
    // 本导出实现只从本库导出：主机身份取 local_origin_host_id；
    // 多主机历史（导入产生）在完整 Merge 排期后扩展为按 host 分包。
    let Some(host_id) = local_host else {
        return Err(CoreError::Validation(
            "export requires an initialized local origin host".into(),
        ));
    };
    let hostname_alias = if request.redact_hostnames {
        None
    } else {
        let name: Option<String> = storage
            .conn()
            .query_row(
                "SELECT hostname FROM origin_host_names WHERE host_id = ?1
                 ORDER BY last_seen_ms DESC LIMIT 1",
                params![host_id],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        name
    };

    // 来源注册：本导出涉及的实例（含其 origin_host_id 关联的原始注册信息）。
    let instance_filter = if request.instances.is_empty() {
        None
    } else {
        Some(request.instances.clone())
    };
    let mut sources = Vec::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT instance_id, agent, format, parser_version, locality_basis,
                    attribution_status, created_at_ms, capabilities
             FROM source_instances",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;
        for row in rows {
            let (
                instance_id,
                agent,
                format,
                parser_version,
                locality_basis,
                attribution_status,
                created_at_ms,
                capabilities,
            ) = row?;
            if let Some(list) = &instance_filter {
                if !list.contains(&instance_id) {
                    continue;
                }
            }
            let completeness = capabilities
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .map(|c| c.get("fields").cloned().unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null);
            sources.push(ExchangeSource {
                source_instance_id: instance_id,
                agent,
                format: format.unwrap_or_default(),
                parser_version: parser_version.unwrap_or_default(),
                locality_basis,
                attribution_status,
                first_seen_ms: created_at_ms,
                completeness,
            });
        }
    }

    // 事件明细。
    let mut records = Vec::new();
    let wanted: BTreeSet<String> = sources
        .iter()
        .map(|s| s.source_instance_id.clone())
        .collect();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT source_instance_id, source_record_key, record_kind, schema_version,
                    parser_version, parse_basis, occurred_at_ms, source_time, time_basis,
                    agent, call_category, lifecycle, provider_id, model_raw, origin_call_id,
                    session_id, parent_session_id, input_uncached, input_cache_read,
                    input_cache_write, input_total, output_total, output_reasoning,
                    total_tokens, source_total, quality_bucket, source_revision, conflict
             FROM usage_events
             WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2",
        )?;
        let rows = stmt.query_map(params![request.from_ms, request.to_ms], |r| {
            Ok(ExchangeRecord {
                source_instance_id: r.get(0)?,
                source_record_key: r.get(1)?,
                record_kind: r.get(2)?,
                schema_version: r.get(3)?,
                parser_version: r.get(4)?,
                parse_basis: r.get(5)?,
                occurred_at_ms: r.get(6)?,
                source_time: r.get(7)?,
                time_basis: r.get(8)?,
                agent: r.get(9)?,
                call_category: r.get(10)?,
                lifecycle: r.get(11)?,
                provider_id: r.get(12)?,
                model_raw: r.get(13)?,
                origin_call_id: r.get(14)?,
                session_id: r.get(15)?,
                parent_session_id: r.get(16)?,
                usage: ExchangeUsage {
                    input_uncached: r.get(17)?,
                    input_cache_read: r.get(18)?,
                    input_cache_write: r.get(19)?,
                    input_total: r.get(20)?,
                    output_total: r.get(21)?,
                    output_reasoning: r.get(22)?,
                    total_tokens: r.get(23)?,
                    source_total: r.get(24)?,
                },
                quality_bucket: r.get(25)?,
                source_revision: r.get(26)?,
                conflict: r.get::<_, i64>(27)? != 0,
                parse_note: None,
            })
        })?;
        for row in rows {
            let record = row?;
            if wanted.contains(&record.source_instance_id) {
                records.push(record);
            }
        }
    }

    // 封存日分区（明细可能已清理，分区保留来源与修订）。
    let mut partitions = Vec::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT tz_version, local_day, instance_id, agent, provider_id, model_raw,
                    call_category, quality_bucket, event_count, call_count,
                    input_known_sum, cache_read_known_sum, cache_write_known_sum,
                    output_known_sum, total_known_sum, conflict_count, sealed, data_revision
             FROM daily_usage WHERE sealed = 1",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ExchangeDailyPartition {
                tz_version: r.get(0)?,
                local_day: r.get(1)?,
                instance_id: r.get(2)?,
                agent: r.get(3)?,
                provider_id: r.get(4)?,
                model_raw: r.get(5)?,
                call_category: r.get(6)?,
                quality_bucket: r.get(7)?,
                event_count: r.get(8)?,
                call_count: r.get(9)?,
                input_known_sum: r.get(10)?,
                cache_read_known_sum: r.get(11)?,
                cache_write_known_sum: r.get(12)?,
                output_known_sum: r.get(13)?,
                total_known_sum: r.get(14)?,
                conflict_count: r.get(15)?,
                sealed: r.get::<_, i64>(16)? != 0,
                data_revision: r.get(17)?,
            })
        })?;
        for row in rows {
            let p = row?;
            if wanted.contains(&p.instance_id) {
                partitions.push(p);
            }
        }
    }

    // 小时层（有界：仅现存的；导入按修订合并）。
    let mut hourly = Vec::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT tz_version, local_day, hour, instance_id, agent, provider_id, model_raw,
                    call_category, quality_bucket, event_count, call_count,
                    input_known_sum, cache_read_known_sum, cache_write_known_sum,
                    output_known_sum, total_known_sum, data_revision
             FROM hourly_usage",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ExchangeHourlyPartition {
                tz_version: r.get(0)?,
                local_day: r.get(1)?,
                hour: r.get(2)?,
                instance_id: r.get(3)?,
                agent: r.get(4)?,
                provider_id: r.get(5)?,
                model_raw: r.get(6)?,
                call_category: r.get(7)?,
                quality_bucket: r.get(8)?,
                event_count: r.get(9)?,
                call_count: r.get(10)?,
                input_known_sum: r.get(11)?,
                cache_read_known_sum: r.get(12)?,
                cache_write_known_sum: r.get(13)?,
                output_known_sum: r.get(14)?,
                total_known_sum: r.get(15)?,
                data_revision: r.get(16)?,
            })
        })?;
        for row in rows {
            let h = row?;
            if wanted.contains(&h.instance_id) {
                hourly.push(h);
            }
        }
    }

    Ok(ExchangeExport {
        format_version: EXCHANGE_FORMAT_VERSION.to_string(),
        kind: request.kind.clone(),
        batch_id: request.batch_id.clone(),
        exported_at_ms: now_ms,
        timezone: request.timezone.clone(),
        host: ExchangeHost {
            origin_host_id: host_id,
            hostname_alias,
        },
        sources,
        records,
        daily_partitions: partitions,
        hourly_partitions: hourly,
    })
}

/// 合并判定输入：现存记录元数据（导入侧从库中读出）。
#[derive(Debug, Clone)]
pub struct ExistingRecord {
    pub source_instance_id: String,
    pub source_record_key: String,
    pub lifecycle: crate::domain::Lifecycle,
    pub source_revision: Option<i64>,
    pub content_hash: String,
}

/// 合并判定结论（data-contract.md 合并规则表）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeDecision {
    /// 新来源/新记录键（覆盖互斥）：新增独立贡献；
    /// 跨源镜像仍按事件仲裁去重，不因来源 ID 不同就直接相加。
    AddIndependent,
    /// 同一来源记录键、同修订且内容一致：幂等跳过，重复导出/导入不增量。
    SkipIdempotent,
    /// 更权威修订（或修订同级的更正）：撤销旧贡献后替换；
    /// 缺少修订顺序证据时不按 token 大小裁决，落 Conflict。
    ReplaceAfterRevoke,
    /// 先后权威关系不可判定且内容不同：保留现存并标记冲突。
    Conflict,
}

/// 逐记录合并判定：复用 ingest 的仲裁（修订号优先，其次生命周期；
/// 同层级内容不同 → 冲突），保证交换判定与本地写入语义一致。
pub fn decide_record_merge(
    incoming: &ExchangeRecord,
    incoming_content_hash: &str,
    incoming_lifecycle: crate::domain::Lifecycle,
    existing: Option<&ExistingRecord>,
) -> MergeDecision {
    let Some(existing) = existing else {
        return MergeDecision::AddIndependent;
    };
    let same_identity = existing.source_instance_id == incoming.source_instance_id
        && existing.source_record_key == incoming.source_record_key;
    if !same_identity {
        return MergeDecision::AddIndependent;
    }
    let meta = ExistingMeta {
        lifecycle: existing.lifecycle,
        source_revision: existing.source_revision,
        content_hash: existing.content_hash.clone(),
    };
    match arbitrate(
        Some(&meta),
        &arbitration_event(incoming, incoming_lifecycle),
        incoming_content_hash,
    ) {
        Arbitration::Insert => MergeDecision::AddIndependent,
        Arbitration::Replace => MergeDecision::ReplaceAfterRevoke,
        Arbitration::Keep => {
            if meta.content_hash == incoming_content_hash {
                MergeDecision::SkipIdempotent
            } else {
                MergeDecision::Conflict
            }
        }
        Arbitration::Conflict => MergeDecision::Conflict,
    }
}

/// 仲裁探针事件：合并判定只关心身份/修订/生命周期/内容哈希，
/// 借用 arbitrate 需要一个 EventInput；usage 不参与仲裁，置空即可。
fn arbitration_event(
    incoming: &ExchangeRecord,
    lifecycle: crate::domain::Lifecycle,
) -> crate::domain::EventInput {
    crate::domain::EventInput {
        source_instance_id: incoming.source_instance_id.clone(),
        source_record_key: incoming.source_record_key.clone(),
        record_kind: crate::domain::RecordKind::ModelCall,
        schema_version: incoming.schema_version.clone(),
        parser_version: incoming.parser_version.clone(),
        parse_basis: None,
        origin_call_id: None,
        attempt_id: None,
        session_id: None,
        parent_session_id: None,
        host_application: None,
        agent: incoming.agent.clone(),
        call_category: crate::domain::CallCategory::Primary,
        occurred_at_ms: incoming.occurred_at_ms,
        observed_at_ms: None,
        source_time: None,
        time_basis: crate::domain::TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: None,
        model_canonical: None,
        model_attribution: crate::domain::ModelAttribution::Unknown,
        usage: crate::domain::TokenUsage::default(),
        quality: crate::domain::TokenQuality::default(),
        lifecycle,
        source_revision: incoming.source_revision,
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: crate::domain::AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }
}
