//! Identity and deduplication: event_id namespaces source instance/record key; content hashes
//! detect changes; revision/lifecycle comparisons select streamed, final or corrected records.
//!
//! Compare both source revisions when present; otherwise partial < final < corrected.
//! Unordered differing content retains the existing record with conflict diagnostics, never MAX.

use crate::domain::{EventInput, Lifecycle};
use serde::Serialize;

/// event_id joins source instance and record key. The composite unique constraint
/// (source_instance_id, source_record_key) identifies the record; this string supports logs/references.
pub fn event_id(source_instance_id: &str, source_record_key: &str) -> String {
    let escape = |s: &str| s.replace('%', "%25").replace('#', "%23");
    format!(
        "{}#{}",
        escape(source_instance_id),
        escape(source_record_key)
    )
}

/// Observation time changes on rescanning without changing source content.
/// parse_basis records known_version/latest_fallback. Registry verification upgrades
/// do not change source content, so this field is also excluded from the content hash.
pub fn event_content_hash(event: &EventInput) -> String {
    let mut content = event.clone();
    content.observed_at_ms = None;
    content.parse_basis = None;
    content_hash(&content)
}

/// FNV-1a 64-bit content hash detects changes; it cannot identify an event independently.
pub fn content_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Existing metadata used to compare incoming records.
#[derive(Debug, Clone)]
pub struct ExistingMeta {
    pub lifecycle: Lifecycle,
    pub source_revision: Option<i64>,
    pub content_hash: String,
}

/// Outcome of comparing a record with existing content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arbitration {
    /// No existing record: insert.
    Insert,
    /// A higher revision/lifecycle replaces the old contribution with the new values.
    Replace,
    /// Keep the higher existing revision/lifecycle or identical content without duplicate usage.
    Keep,
    /// Unordered different content: retain the existing record, mark conflict and diagnose.
    Conflict,
}

/// Compare source revisions before lifecycle; equal precedence with different content is conflict.
pub fn arbitrate(
    existing: Option<&ExistingMeta>,
    incoming: &EventInput,
    incoming_hash: &str,
) -> Arbitration {
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
    // Missing revision on either side uses lifecycle; equal lifecycle/content keeps, differing content conflicts.
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
            parse_basis: None,
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
        ExistingMeta {
            lifecycle,
            source_revision: revision,
            content_hash: hash.into(),
        }
    }

    #[test]
    fn revision_orders_first() {
        let e = sample_event(Some(7), Lifecycle::Partial);
        // A higher source revision replaces even with a lower lifecycle.
        assert_eq!(
            arbitrate(Some(&meta(Some(6), Lifecycle::Final, "a")), &e, "b"),
            Arbitration::Replace
        );
        // A lower source revision retains the existing record.
        assert_eq!(
            arbitrate(Some(&meta(Some(8), Lifecycle::Partial, "a")), &e, "b"),
            Arbitration::Keep
        );
        // Equal revision and content adds no usage.
        assert_eq!(
            arbitrate(Some(&meta(Some(7), Lifecycle::Partial, "same")), &e, "same"),
            Arbitration::Keep
        );
        // Equal revision with different content is conflict.
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
            arbitrate(
                Some(&meta(None, Lifecycle::Final, "same")),
                &sample_event(None, Lifecycle::Final),
                "same"
            ),
            Arbitration::Keep
        );
        assert_eq!(
            arbitrate(
                Some(&meta(None, Lifecycle::Final, "x")),
                &sample_event(None, Lifecycle::Final),
                "y"
            ),
            Arbitration::Conflict
        );
        assert_eq!(arbitrate(None, &partial, "h"), Arbitration::Insert);
    }
}
