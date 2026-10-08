//! Convert the models.dev api.json catalog to price snapshots for F2 online refresh.
//! Rules: docs/design/desktop-usage/pricing.md, online refresh design.
//!
//! S24 structure observed on 2026-10-01: provider_id maps to id/name/doc/env/
//! npm/models, with model_id mapping to ModelEntry. Its cost fields include
//! input/output/cache_read/cache_write/input_audio/output_audio/reasoning/
//! tiers/context_over_200k, denominated in USD per million tokens without a currency field.
//! canonical_model_id, such as openai/gpt-6-astra, supplies catalog provider attribution.
//!
//! Select official-provider pay-as-you-go entries by the catalog rules:
//! - provider IDs must match a canonical_model_id prefix or its -cn variant;
//!   exclude IDs containing -plan, including coding-plan/token-plan subscription
//!   placeholders whose observed costs were all zero;
//! - skip models with input/output both zero or absent; subscription placeholders are not prices;
//! - raw zero components become NULL, without treating placeholder zeros as free rates;
//! - map cache_write to cache_write_5m because the catalog lacks TTL tiers; context-type tiers
//!   become context_threshold_tokens rows, without duplicating context_over_200k;
//!   do not import audio/reasoning rates separately; reasoning remains within output pricing;
//! - set region cn for -cn variants, otherwise global; channel api, currency USD,
//!   service_tier standard and official_vendor true;
//! - convert USD/million tokens to hundredths of a cent/million with multiplication by 10^4 and rounding.

use crate::error::CoreError;
use crate::pricing::{PriceRow, PriceSnapshot};
use jiff::tz::TimeZone;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Only built-in online price source; HTTPS GET sends no local data.
pub const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

/// Response-size limit; S24 observed about 5.3 MiB.
pub const MODELS_DEV_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct ApiFile {
    #[serde(flatten)]
    providers: BTreeMap<String, ProviderEntry>,
}

#[derive(Debug, Deserialize)]
struct ProviderEntry {
    #[serde(default)]
    models: BTreeMap<String, ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    cost: Option<ModelCost>,
    canonical_model_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct ModelCost {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    #[serde(default)]
    tiers: Vec<CostTier>,
}

#[derive(Debug, Deserialize, Clone)]
struct CostTier {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    tier: Option<TierMeta>,
}

#[derive(Debug, Deserialize, Clone)]
struct TierMeta {
    #[serde(rename = "type")]
    kind: String,
    size: Option<i64>,
}

/// Convert USD/million tokens to hundredths of a cent/million by 10^4 and rounding.
/// Raw zero placeholders become None rather than free prices.
fn usd_to_hundredths(value: Option<f64>) -> Option<i64> {
    match value {
        Some(v) if v.is_finite() && v > 0.0 => {
            let scaled = v * 10_000.0;
            if scaled > i64::MAX as f64 {
                return None;
            }
            Some((scaled + 0.5) as i64)
        }
        Some(v) if v.is_finite() && v == 0.0 => None,
        Some(_) => None, // Skip negative/non-finite raw rates before snapshot validation.
        None => None,
    }
}

/// Select provider IDs from canonical_model_id prefixes and their -cn variants;
/// exclude -plan subscription placeholders.
fn official_provider_ids(api: &ApiFile) -> BTreeSet<String> {
    let mut prefixes: BTreeSet<String> = BTreeSet::new();
    for provider in api.providers.values() {
        for model in provider.models.values() {
            if let Some(canonical) = &model.canonical_model_id {
                if let Some((prefix, _)) = canonical.split_once('/') {
                    if !prefix.is_empty() {
                        prefixes.insert(prefix.to_string());
                    }
                }
            }
        }
    }
    api.providers
        .keys()
        .filter(|id| {
            if id.contains("-plan") {
                return false;
            }
            if prefixes.contains(id.as_str()) {
                return true;
            }
            id.strip_suffix("-cn")
                .is_some_and(|base| prefixes.contains(base))
        })
        .map(String::clone)
        .collect()
}

/// Convert raw api.json to a community price snapshot (source_type=community).
/// Successful fetched_at_ms determines the snapshot date and effective start.
/// Raw-response FNV-1a hash contributes to snapshot identity and duplicate detection.
pub fn snapshot_from_models_dev(
    json_text: &str,
    fetched_at_ms: i64,
    raw_hash: u64,
) -> Result<PriceSnapshot, CoreError> {
    let api: ApiFile = serde_json::from_str(json_text)
        .map_err(|e| CoreError::Validation(format!("models.dev api.json parse failed: {e}")))?;
    if api.providers.is_empty() {
        return Err(CoreError::Validation(
            "models.dev api.json: no providers".into(),
        ));
    }
    let official = official_provider_ids(&api);
    if official.is_empty() {
        return Err(CoreError::Validation(
            "models.dev api.json: no official providers resolved".into(),
        ));
    }

    let fetched_date = jiff::Timestamp::from_millisecond(fetched_at_ms)
        .map_err(|e| CoreError::Validation(format!("models.dev fetched_at: {e}")))?
        .to_zoned(TimeZone::UTC)
        .date();
    let snapshot_id = format!("models-dev-{}-{:08x}", fetched_date, raw_hash as u32);
    let effective_from = fetched_date.to_string();
    // Use the fetched UTC date as effective start because the catalog has no rate-effective intervals.
    let effective_from_ms = crate::pricing::iso_date_to_ms(&effective_from, "effective_from")?;

    let mut rows: Vec<PriceRow> = Vec::new();
    for provider_id in &official {
        let provider = &api.providers[provider_id];
        let region = if provider_id.ends_with("-cn") {
            "cn"
        } else {
            "global"
        };
        for (model_id, model) in &provider.models {
            let Some(cost) = &model.cost else { continue };
            let input = usd_to_hundredths(cost.input);
            let output = usd_to_hundredths(cost.output);
            if input.is_none() && output.is_none() {
                continue; // Skip subscription placeholders or entries without usable rates.
            }
            let base_id = format!("{snapshot_id}:{provider_id}:{model_id}");
            let mut push_row = |threshold: i64,
                                input: Option<i64>,
                                cache_read: Option<i64>,
                                cache_write_5m: Option<i64>,
                                output: Option<i64>,
                                suffix: String| {
                rows.push(PriceRow {
                    price_id: format!("{base_id}{suffix}"),
                    snapshot_id: snapshot_id.clone(),
                    provider_id: provider_id.clone(),
                    model: model_id.clone(),
                    region: region.to_string(),
                    channel: "api".to_string(),
                    service_tier: "standard".to_string(),
                    context_threshold_tokens: threshold,
                    effective_from_ms,
                    effective_to_ms: None,
                    input_per_mtok_hundredths: input,
                    cache_read_per_mtok_hundredths: cache_read,
                    cache_write_5m_per_mtok_hundredths: cache_write_5m,
                    cache_write_1h_per_mtok_hundredths: None,
                    output_per_mtok_hundredths: output,
                    cache_storage_per_mtok_hour_hundredths: None,
                    currency: "USD".to_string(),
                    official_vendor: true,
                    note: Some(
                        "models.dev 社区目录按量列表价（USD/Mtok 折算；分项 0 按无价处理）"
                            .to_string(),
                    ),
                });
            };
            push_row(
                0,
                input,
                usd_to_hundredths(cost.cache_read),
                usd_to_hundredths(cost.cache_write),
                output,
                String::new(),
            );
            // Import context-type long-context tiers, keeping the first entry for each size.
            let mut seen_sizes: BTreeSet<i64> = BTreeSet::new();
            for tier in &cost.tiers {
                let Some(meta) = &tier.tier else { continue };
                if meta.kind != "context" {
                    continue;
                }
                let Some(size) = meta.size else { continue };
                if size <= 0 || !seen_sizes.insert(size) {
                    continue;
                }
                let t_input = usd_to_hundredths(tier.input);
                let t_output = usd_to_hundredths(tier.output);
                if t_input.is_none() && t_output.is_none() {
                    continue;
                }
                push_row(
                    size,
                    t_input,
                    usd_to_hundredths(tier.cache_read),
                    usd_to_hundredths(tier.cache_write),
                    t_output,
                    format!(":t{size}"),
                );
            }
        }
    }
    if rows.is_empty() {
        return Err(CoreError::Validation(
            "models.dev api.json: no priced official rows after filtering".into(),
        ));
    }
    rows.sort_by(|a, b| a.price_id.cmp(&b.price_id));

    // Reuse SnapshotFile validation for intervals, nonnegative values and enums before normalization.
    let file = crate::pricing::SnapshotFile {
        format: "llm-usage-price-snapshot/1".to_string(),
        snapshot: crate::pricing::SnapshotMetaFile {
            id: snapshot_id.clone(),
            source_type: "community".to_string(),
            source_urls: vec![MODELS_DEV_API_URL.to_string()],
            fetched_at: effective_from.clone(),
            verified_at: None,
            verified_by: Some(
                "models.dev 过滤链：官方提供商（canonical 前缀 ∪ -cn 变体）+ 排除订阅占位"
                    .to_string(),
            ),
            license: Some("MIT".to_string()),
            note: Some(format!(
                "models.dev api.json 在线刷新导入；官方提供商 {} 个，价格行 {}；\
                 CN 渠道人民币价不在目录内（种子/手工快照提供）",
                official.len(),
                rows.len()
            )),
        },
        rows: rows
            .iter()
            .map(|r| crate::pricing::PriceRowFile {
                price_id: r.price_id.clone(),
                provider_id: r.provider_id.clone(),
                model: r.model.clone(),
                region: r.region.clone(),
                channel: r.channel.clone(),
                service_tier: r.service_tier.clone(),
                context_threshold_tokens: Some(r.context_threshold_tokens),
                effective_from: effective_from.clone(),
                effective_to: None,
                currency: r.currency.clone(),
                input: r.input_per_mtok_hundredths,
                cache_read: r.cache_read_per_mtok_hundredths,
                cache_write_5m: r.cache_write_5m_per_mtok_hundredths,
                cache_write_1h: None,
                output: r.output_per_mtok_hundredths,
                cache_storage_hour: None,
                official_vendor: Some(true),
                note: r.note.clone(),
            })
            .collect(),
    };
    PriceSnapshot::from_file(&file)
}

/// Raw-response FNV-1a hash uses the pricing algorithm for snapshot identity.
pub fn content_hash(bytes: &[u8]) -> u64 {
    crate::pricing::fnv1a(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic catalog includes canonical vendorA, its vendorA-cn variant,
    /// subscription placeholders, an aggregator, zero-rate models, context tiers and zero components.
    const MINI: &str = r#"{
      "vendorA": {
        "id": "vendorA",
        "models": {
          "m-one": {
            "id": "m-one",
            "canonical_model_id": "vendorA/m-one",
            "cost": {"input": 10, "output": 50, "cache_read": 1, "cache_write": 12.5,
              "tiers": [{"input": 20, "output": 75, "cache_read": 2, "cache_write": 25,
                         "tier": {"type": "context", "size": 272000}}],
              "context_over_200k": {"input": 20, "output": 75}}
          },
          "m-zero": {
            "id": "m-zero",
            "canonical_model_id": "vendorA/m-zero",
            "cost": {"input": 0, "output": 0, "cache_read": 0}
          },
          "m-free-cache": {
            "id": "m-free-cache",
            "canonical_model_id": "vendorA/m-free-cache",
            "cost": {"input": 1.4, "output": 4.4, "cache_read": 0.26, "cache_write": 0}
          },
          "m-tiny": {
            "id": "m-tiny",
            "canonical_model_id": "vendorA/m-tiny",
            "cost": {"input": 0.435, "output": 0.87, "cache_read": 0.003625}
          }
        }
      },
      "vendorA-cn": {
        "id": "vendorA-cn",
        "models": {
          "m-one": {
            "id": "m-one",
            "canonical_model_id": "vendorA/m-one",
            "cost": {"input": 0.115, "output": 0.287}
          }
        }
      },
      "vendorA-coding-plan": {
        "id": "vendorA-coding-plan",
        "models": {
          "m-one": {"id": "m-one", "cost": {"input": 0, "output": 0}}
        }
      },
      "aggregator": {
        "id": "aggregator",
        "models": {
          "m-one": {
            "id": "vendorA/m-one",
            "canonical_model_id": "vendorA/m-one",
            "cost": {"input": 9, "output": 45}
          }
        }
      }
    }"#;

    fn convert() -> PriceSnapshot {
        let hash = content_hash(MINI.as_bytes());
        snapshot_from_models_dev(MINI, 1_790_000_000_000, hash).expect("mini converts")
    }

    #[test]
    fn filters_to_official_pay_as_you_go_rows() {
        let snapshot = convert();
        // vendorA: m-one base and 272K tier, m-free-cache and m-tiny; vendorA-cn: m-one.
        // Exclude all-zero m-zero and coding-plan/aggregator entries.
        assert_eq!(snapshot.rows.len(), 5);
        assert!(snapshot
            .rows
            .iter()
            .all(|r| r.official_vendor && r.currency == "USD" && r.channel == "api"));
        assert!(snapshot
            .rows
            .iter()
            .all(|r| r.provider_id == "vendorA" || r.provider_id == "vendorA-cn"));
        assert!(!snapshot.rows.iter().any(|r| r.model == "m-zero"));
        // -cn variants use region cn; other entries use global.
        let cn = snapshot
            .rows
            .iter()
            .find(|r| r.provider_id == "vendorA-cn")
            .expect("cn row");
        assert_eq!(cn.region, "cn");
        assert_eq!(cn.input_per_mtok_hundredths, Some(1150)); // 0.115 * 10^4.
    }

    #[test]
    fn converts_units_and_tiers() {
        let snapshot = convert();
        let base = snapshot
            .rows
            .iter()
            .find(|r| r.price_id.ends_with(":vendorA:m-one"))
            .expect("base row");
        assert_eq!(base.input_per_mtok_hundredths, Some(100_000)); // USD 10 per million tokens.
        assert_eq!(base.cache_write_5m_per_mtok_hundredths, Some(125_000)); // USD 12.5 per million tokens, in the five-minute tier.
        assert_eq!(base.cache_write_1h_per_mtok_hundredths, None); // The catalog has no one-hour tier.
        assert_eq!(base.context_threshold_tokens, 0);
        let tier = snapshot
            .rows
            .iter()
            .find(|r| r.price_id.ends_with(":t272000"))
            .expect("tier row");
        assert_eq!(tier.context_threshold_tokens, 272000);
        assert_eq!(tier.output_per_mtok_hundredths, Some(750_000)); // USD 75 per million tokens.
                                                                    // Raw zero components become NULL instead of free placeholder rates.
        let free_cache = snapshot
            .rows
            .iter()
            .find(|r| r.model == "m-free-cache")
            .expect("free cache row");
        assert_eq!(free_cache.cache_write_5m_per_mtok_hundredths, None);
        assert_eq!(free_cache.cache_read_per_mtok_hundredths, Some(2600));
        // Round USD 0.003625/M * 10^4 = 36.25 to 36.
        let tiny = snapshot
            .rows
            .iter()
            .find(|r| r.model == "m-tiny")
            .expect("tiny row");
        assert_eq!(tiny.cache_read_per_mtok_hundredths, Some(36));
    }

    #[test]
    fn snapshot_id_is_content_addressed_and_deterministic() {
        let a = convert();
        let b = convert();
        assert_eq!(a.snapshot_id, b.snapshot_id);
        assert!(a.snapshot_id.starts_with("models-dev-"));
        assert_eq!(a.source_type, "community");
        assert_eq!(a.source_urls, [MODELS_DEV_API_URL]);
        assert_eq!(a.license.as_deref(), Some("MIT"));
        // Changed bytes produce a different snapshot ID, as required for corrected snapshots.
        let changed = MINI.replace("\"input\": 10", "\"input\": 11");
        let other = snapshot_from_models_dev(
            &changed,
            1_790_000_000_000,
            content_hash(changed.as_bytes()),
        )
        .expect("changed converts");
        assert_ne!(a.snapshot_id, other.snapshot_id);
    }

    #[test]
    fn rejects_invalid_and_empty_content() {
        assert!(snapshot_from_models_dev("not json", 0, 0).is_err());
        assert!(snapshot_from_models_dev("{}", 0, 0).is_err());
        // Subscription-only zero placeholders yield no rows and must be rejected.
        let only_plan =
            r#"{"p-coding-plan": {"models": {"m": {"cost": {"input": 0, "output": 0}}}}}"#;
        assert!(snapshot_from_models_dev(only_plan, 0, 0).is_err());
    }
}
