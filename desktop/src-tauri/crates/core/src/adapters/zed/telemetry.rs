use super::common::ZedUsage;
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, FieldQuality, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, TokenQuality, TokenUsage, MAX_TOKEN_VALUE,
};
use serde_json::Value;
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

pub const PARSER: &str = "zed-local-telemetry-1";
type Model = (String, String);

#[derive(Default)]
struct Thread {
    bad: bool,
    last: Option<(String, Model, Option<ZedUsage>)>,
    sums: BTreeMap<Model, ZedUsage>,
    paired_calls: BTreeMap<Model, BTreeMap<String, usize>>,
}

#[derive(Default)]
pub(super) struct Log(BTreeMap<String, Thread>);

fn add(a: ZedUsage, b: ZedUsage) -> Option<ZedUsage> {
    let sum = |a: i64, b: i64| a.checked_add(b).filter(|v| *v <= MAX_TOKEN_VALUE);
    Some(ZedUsage {
        input_tokens: sum(a.input_tokens, b.input_tokens)?,
        output_tokens: sum(a.output_tokens, b.output_tokens)?,
        cache_read_input_tokens: sum(a.cache_read_input_tokens, b.cache_read_input_tokens)?,
        cache_creation_input_tokens: sum(
            a.cache_creation_input_tokens,
            b.cache_creation_input_tokens,
        )?,
    })
}

fn key(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
}

fn model(p: &Value) -> Option<Model> {
    let provider = key(&p["model_provider"])?;
    // Both Zed OpenAI mappers construct "openai/" + the original model ID.
    let model = key(&p["model"])?
        .strip_prefix("openai/")
        .filter(|s| !s.is_empty())?;
    Some((provider.into(), model.into()))
}

fn usage(p: &Value) -> Option<ZedUsage> {
    let number = |name: &str| {
        p[name]
            .as_i64()
            .filter(|n| (0..=MAX_TOKEN_VALUE).contains(n))
    };
    let row = ZedUsage {
        input_tokens: number("input_tokens")?,
        output_tokens: number("output_tokens")?,
        cache_read_input_tokens: number("cache_read_input_tokens")?,
        cache_creation_input_tokens: number("cache_creation_input_tokens")?,
    };
    // A positive residual proves the subtraction did not saturate.
    (row.input_tokens > 0 && row.output_tokens > 0).then_some(row)
}

impl Log {
    pub(super) fn read(db: &Path) -> Option<Self> {
        const LIMIT: u64 = 8 * 1024 * 1024;
        let threads = db.parent()?;
        if db.file_name()?.to_str()? != "threads.db" || threads.file_name()?.to_str()? != "threads"
        {
            return None;
        }
        let path = threads.parent()?.join("logs").join("telemetry.log");
        if !path.is_file()
            || std::fs::symlink_metadata(&path)
                .ok()?
                .file_type()
                .is_symlink()
        {
            return None;
        }
        let mut input = Vec::new();
        crate::adapters::run_policy::checked_reader(File::open(path).ok()?)
            .take(LIMIT + 1)
            .read_to_end(&mut input)
            .ok()?;
        if input.len() as u64 > LIMIT || input.last() != Some(&b'\n') {
            return None;
        }
        let mut log = Self::default();
        for line in input[..input.len() - 1].split(|b| *b == b'\n') {
            let v: Value = crate::adapters::run_policy::json_from_slice(line).ok()?;
            let kind = v["event_type"].as_str().unwrap_or_default();
            if !matches!(
                kind,
                "Agent Thread Completion" | "Agent Thread Completion Usage Updated"
            ) {
                continue;
            }
            let p = &v["event_properties"];
            let thread = log.0.entry(key(&p["thread_id"])?.into()).or_default();
            if thread.bad {
                continue;
            }
            let result = (|| {
                if v["type"] != "Flexible"
                    || p["event_source"] != "zed"
                    || v["milliseconds_since_first_event"].as_u64().is_none()
                {
                    return None;
                }
                let prompt = key(&p["prompt_id"])?;
                let id = model(p)?;
                if kind == "Agent Thread Completion" {
                    p["attempt"].as_u64()?;
                    thread.last = Some((prompt.into(), id, None));
                    return Some(());
                }
                let (last_prompt, last_id, last_usage) = thread.last.as_mut()?;
                if last_prompt != prompt || *last_id != id {
                    return None;
                }
                let u = usage(p)?;
                if let Some(prior) = last_usage {
                    return (*prior == u).then_some(());
                }
                *last_usage = Some(u);
                *thread
                    .paired_calls
                    .entry(id.clone())
                    .or_default()
                    .entry(prompt.into())
                    .or_default() += 1;
                let total = thread.sums.entry(id).or_default();
                *total = add(*total, u)?;
                Some(())
            })();
            thread.bad = result.is_none();
        }
        Some(log)
    }

    pub(super) fn events(
        &self,
        instance: &str,
        thread_id: &str,
        start: i64,
        end: i64,
        native: ZedUsage,
    ) -> Vec<EventInput> {
        let Some(thread) = self
            .0
            .get(thread_id)
            .filter(|t| !t.bad && !t.sums.is_empty())
        else {
            return Vec::new();
        };
        if start >= end
            || thread
                .sums
                .values()
                .copied()
                .try_fold(ZedUsage::default(), add)
                != Some(native)
        {
            return Vec::new();
        }
        thread
            .sums
            .iter()
            .filter_map(|(id, raw)| {
                let (provider, model) = id;
                let known = |v| (v > 0).then_some(v);
                let q = |v: Option<i64>| {
                    if v.is_some() {
                        FieldQuality::Reported
                    } else {
                        FieldQuality::Unknown
                    }
                };
                let input_total = raw
                    .input_tokens
                    .checked_add(raw.cache_read_input_tokens)?
                    .checked_add(raw.cache_creation_input_tokens)?;
                let total = input_total.checked_add(raw.output_tokens)?;
                if total > MAX_TOKEN_VALUE {
                    return None;
                }
                let usage = TokenUsage {
                    input_uncached: known(raw.input_tokens),
                    input_cache_read: known(raw.cache_read_input_tokens),
                    input_cache_write: known(raw.cache_creation_input_tokens),
                    input_total: Some(input_total),
                    output_total: known(raw.output_tokens),
                    total_tokens: Some(total),
                    ..Default::default()
                };
                let quality = TokenQuality {
                    input_uncached: q(usage.input_uncached),
                    input_cache_read: q(usage.input_cache_read),
                    input_cache_write: q(usage.input_cache_write),
                    input_total: FieldQuality::Derived,
                    output_total: q(usage.output_total),
                    total_tokens: FieldQuality::Derived,
                    ..Default::default()
                };
                let observation = EventInput {
                    source_instance_id: instance.into(),
                    source_record_key: format!(
                        "zed:local-usage:{}",
                        crate::identity::content_hash(&(thread_id, provider, model))
                    ),
                    record_kind: RecordKind::UsageObservation,
                    schema_version: "zed-0.3.0-local-completion".into(),
                    parser_version: PARSER.into(),
                    parse_basis: None,
                    origin_call_id: None,
                    attempt_id: None,
                    session_id: Some(thread_id.into()),
                    parent_session_id: None,
                    host_application: None,
                    agent: "zed".into(),
                    call_category: CallCategory::Unknown,
                    occurred_at_ms: end,
                    observed_at_ms: None,
                    source_time: None,
                    time_basis: TimeBasis::BoundedInterval,
                    interval_start_ms: Some(start),
                    interval_end_ms: Some(end),
                    provider_id: Some(provider.clone()),
                    model_raw: Some(model.clone()),
                    model_canonical: None,
                    model_attribution: ModelAttribution::RequestField,
                    usage,
                    quality,
                    lifecycle: Lifecycle::Final,
                    source_revision: Some(end),
                    error_status: None,
                    duration_ms: None,
                    ttft_ms: None,
                    attribution_status: AttributionStatus::Verified,
                    exclusion_reason: None,
                    cost: None,
                };
                // Each matched start and usage update verifies one request within
                // the thread. Its tokens already belong to the model observation.
                let calls = thread.paired_calls.get(id)?;
                let mut rows = vec![observation.clone()];
                for (prompt, count) in calls {
                    for ordinal in 0..*count {
                        let mut call = observation.clone();
                        call.source_record_key = format!(
                            "zed:local-call:{}",
                            crate::identity::content_hash(&(
                                thread_id, prompt, provider, model, ordinal
                            ))
                        );
                        call.record_kind = RecordKind::ModelCall;
                        call.usage = TokenUsage::default();
                        call.quality = TokenQuality::default();
                        rows.push(call);
                    }
                }
                Some(rows)
            })
            .flatten()
            .collect()
    }
}
