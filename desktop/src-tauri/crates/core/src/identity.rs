//! 身份与去重：event_id 命名空间（源实例+源记录键）、内容哈希（仅变更检测）、
//! 同请求流式/最终/更正的 upsert 仲裁。
//!
//! 合同：有源修订号按修订号排序；否则按生命周期裁决（partial < final < corrected）。
//! 不能确定先后权威关系时标记 conflict，保留诊断，不取 MAX。

use crate::domain::{EventInput, Lifecycle};
use serde::Serialize;

/// event_id = 源实例命名空间 + 源记录键。复合唯一约束 (source_instance_id,
/// source_record_key) 才是权威身份；本字符串便于日志与跨表引用。
pub fn event_id(source_instance_id: &str, source_record_key: &str) -> String {
    format!("{source_instance_id}#{source_record_key}")
}

/// FNV-1a 64 位内容哈希：只用于变更检测，不独立作为事件身份。
pub fn content_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// 已有记录的仲裁元数据。
#[derive(Debug, Clone)]
pub struct ExistingMeta {
    pub lifecycle: Lifecycle,
    pub source_revision: Option<i64>,
    pub content_hash: String,
}

/// upsert 仲裁结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arbitration {
    /// 无现存记录：插入。
    Insert,
    /// 新记录更权威：替换（更正撤销旧贡献再应用新值）。
    Replace,
    /// 现存记录更权威或内容相同：保留（幂等重放）。
    Keep,
    /// 先后权威关系不可判定且内容不同：保留现存，标 conflict + 诊断。
    Conflict,
}

/// 仲裁：修订号优先，其次生命周期；同层级内容不同 → conflict。
pub fn arbitrate(existing: Option<&ExistingMeta>, incoming: &EventInput, incoming_hash: &str) -> Arbitration {
    let Some(existing) = existing else {
        return Arbitration::Insert;
    };
    if let (Some(new_rev), Some(old_rev)) = (incoming.source_revision, existing.source_revision) {
        match new_rev.cmp(&old_rev) {
            std::cmp::Ordering::Greater => return Arbitration::Replace,
            std::cmp::Ordering::Less => return Arbitration::Keep,
            std::cmp::Ordering::Equal => {
                return if existing.content_hash == incoming_hash {
                    Arbitration::Keep
                } else {
                    Arbitration::Conflict
                };
            }
        }
    }
    // 无修订号（或只有一侧有）：按生命周期裁决；同级同内容幂等，同级不同内容冲突。
    match incoming.lifecycle.cmp(&existing.lifecycle) {
        std::cmp::Ordering::Greater => Arbitration::Replace,
        std::cmp::Ordering::Less => Arbitration::Keep,
        std::cmp::Ordering::Equal => {
            if existing.content_hash == incoming_hash {
                Arbitration::Keep
            } else {
                Arbitration::Conflict
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::*;

    fn sample_event(revision: Option<i64>, lifecycle: Lifecycle) -> EventInput {
        EventInput {
            source_instance_id: "inst".into(),
            source_record_key: "k".into(),
            record_kind: RecordKind::ModelCall,
            schema_version: "1".into(),
            parser_version: "1".into(),
            origin_call_id: None,
            attempt_id: None,
            session_id: None,
            parent_session_id: None,
            host_application: None,
            agent: "agent".into(),
            call_category: CallCategory::Primary,
            occurred_at_ms: 1_700_000_000_000,
            observed_at_ms: None,
            source_time: None,
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: TokenUsage::default(),
            quality: TokenQuality::default(),
            lifecycle,
            source_revision: revision,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        }
    }

    fn meta(revision: Option<i64>, lifecycle: Lifecycle, hash: &str) -> ExistingMeta {
        ExistingMeta { lifecycle, source_revision: revision, content_hash: hash.into() }
    }

    #[test]
    fn revision_orders_first() {
        let e = sample_event(Some(7), Lifecycle::Partial);
        // 修订号更高：即使生命周期更低也替换（源明确排序）。
        assert_eq!(
            arbitrate(Some(&meta(Some(6), Lifecycle::Final, "a")), &e, "b"),
            Arbitration::Replace
        );
        // 修订号更低：保留。
        assert_eq!(
            arbitrate(Some(&meta(Some(8), Lifecycle::Partial, "a")), &e, "b"),
            Arbitration::Keep
        );
        // 同修订号同内容：幂等。
        assert_eq!(
            arbitrate(Some(&meta(Some(7), Lifecycle::Partial, "same")), &e, "same"),
            Arbitration::Keep
        );
        // 同修订号不同内容：conflict。
        assert_eq!(
            arbitrate(Some(&meta(Some(7), Lifecycle::Partial, "x")), &e, "y"),
            Arbitration::Conflict
        );
    }

    #[test]
    fn lifecycle_arbitrates_without_revision() {
        let partial = sample_event(None, Lifecycle::Partial);
        let corrected = sample_event(None, Lifecycle::Corrected);
        assert_eq!(
            arbitrate(Some(&meta(None, Lifecycle::Final, "a")), &partial, "b"),
            Arbitration::Keep
        );
        assert_eq!(
            arbitrate(Some(&meta(None, Lifecycle::Final, "a")), &corrected, "b"),
            Arbitration::Replace
        );
        assert_eq!(
            arbitrate(Some(&meta(None, Lifecycle::Final, "same")), &sample_event(None, Lifecycle::Final), "same"),
            Arbitration::Keep
        );
        assert_eq!(
            arbitrate(Some(&meta(None, Lifecycle::Final, "x")), &sample_event(None, Lifecycle::Final), "y"),
            Arbitration::Conflict
        );
        assert_eq!(arbitrate(None, &partial, "h"), Arbitration::Insert);
    }
}
