//! models.dev 社区目录（`api.json`）→ 价格快照转换（F2 在线刷新，
//! [价格规范 · 在线刷新设计](../../../docs/design/desktop-usage/pricing.md)）。
//!
//! 实测结构（2026-10-01，S24）：顶层为 `{ provider_id: { id, name, doc, env,
//! npm, models: { model_id: ModelEntry } } }`；`ModelEntry.cost` 字段集为
//! input/output/cache_read/cache_write/input_audio/output_audio/reasoning/
//! tiers/context_over_200k（美元/百万 token，无币种字段）；`canonical_model_id`
//! 形如 `"openai/gpt-6-astra"`，是目录自身的官方归属标注。
//!
//! 过滤约定（以官方按量价为准）：
//! - 官方提供商 = 至少一个模型 `canonical_model_id` 前缀对应的提供商 ID，
//!   或其 `<id>-cn` 中国区变体；ID 含 `-plan`（coding-plan/token-plan 等订阅
//!   占位，实测 cost 全 0）整体排除；
//! - 模型级 input 与 output 同时为 0/缺失 ⇒ 跳过（订阅占位不是价格）；
//! - 分量 0 值 ⇒ NULL（无价），不把目录占位零值当免费价格；
//! - `cache_write` → `cache_write_5m`（目录无 TTL 分档）；`tiers[type=context]`
//!   → `context_threshold_tokens` 行；`context_over_200k` 为重复表达不采用；
//!   audio/reasoning 价格项不导入（reasoning 仍含在输出价格中）；
//! - 行属性：region = cn（-cn 变体）/ global，channel = api，currency = USD，
//!   service_tier = standard，official_vendor = true；
//! - 单位折算：美元/百万 token ×10⁴ → 百分之一美分/百万 token，四舍五入。

use crate::error::CoreError;
use crate::pricing::{PriceRow, PriceSnapshot};
use jiff::tz::TimeZone;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// 内置在线刷新来源（唯一；HTTPS GET，不携带任何本地数据）。
pub const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

/// 响应体上限（实测约 5.3 MiB；上限防异常膨胀）。
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

/// 美元/百万 token → 百分之一美分/百万 token（×10⁴ 四舍五入；0 ⇒ None，
/// 目录占位零值不当免费价格）。
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
        Some(_) => None, // 负值/非有限值：不导入（校验层对负价的拒绝针对显式负值）
        None => None,
    }
}

/// 官方提供商判定：ID 是任一 canonical_model_id 前缀，或为 `<前缀>-cn` 变体；
/// ID 含 `-plan` 的订阅占位渠道整体排除。
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

/// 把 api.json 原文转换为社区价格快照（source_type=community）。
/// `fetched_at_ms` 为本次成功下载时间（决定快照 ID 日期与行生效起点）。
/// `raw_hash` 为原始响应内容的 FNV-1a 哈希（快照 ID 组成部分，幂等前提）。
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
    // 生效起点 = 抓取日期（UTC 零点）；目录无价格生效区间。
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
                continue; // 订阅占位/无价目：跳过
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
            // 长上下文档：仅 context 类型；同尺寸去重（保首个）。
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

    // 复用快照文件校验链（区间重叠/非负/枚举等）：构造等价 SnapshotFile 再归一化。
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

/// 原始响应内容的 FNV-1a 哈希（与 pricing 模块同算法，快照 ID 用）。
pub fn content_hash(bytes: &[u8]) -> u64 {
    crate::pricing::fnv1a(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 迷你 api.json：vendorA（canonical 前缀官方）、vendora-cn（-cn 变体）、
    /// coding-plan 占位、聚合商（非官方）、零价模型、长上下文档、0 分量。
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
        // vendorA：m-one 基价 + 272K 档、m-free-cache、m-tiny；vendorA-cn：m-one。
        // m-zero（全 0）与 coding-plan/aggregator 条目被排除。
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
        // -cn 变体 region=cn，其余 global。
        let cn = snapshot
            .rows
            .iter()
            .find(|r| r.provider_id == "vendorA-cn")
            .expect("cn row");
        assert_eq!(cn.region, "cn");
        assert_eq!(cn.input_per_mtok_hundredths, Some(1150)); // 0.115 × 10⁴
    }

    #[test]
    fn converts_units_and_tiers() {
        let snapshot = convert();
        let base = snapshot
            .rows
            .iter()
            .find(|r| r.price_id.ends_with(":vendorA:m-one"))
            .expect("base row");
        assert_eq!(base.input_per_mtok_hundredths, Some(100_000)); // $10/M
        assert_eq!(base.cache_write_5m_per_mtok_hundredths, Some(125_000)); // $12.5/M → 5m 档
        assert_eq!(base.cache_write_1h_per_mtok_hundredths, None); // 目录无 1h 档
        assert_eq!(base.context_threshold_tokens, 0);
        let tier = snapshot
            .rows
            .iter()
            .find(|r| r.price_id.ends_with(":t272000"))
            .expect("tier row");
        assert_eq!(tier.context_threshold_tokens, 272000);
        assert_eq!(tier.output_per_mtok_hundredths, Some(750_000)); // $75/M
                                                                    // 0 分量 ⇒ NULL（不把占位零值当免费价格）。
        let free_cache = snapshot
            .rows
            .iter()
            .find(|r| r.model == "m-free-cache")
            .expect("free cache row");
        assert_eq!(free_cache.cache_write_5m_per_mtok_hundredths, None);
        assert_eq!(free_cache.cache_read_per_mtok_hundredths, Some(2600));
        // 极低分项四舍五入：$0.003625/M ×10⁴ = 36.25 → 36。
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
        // 内容变化 ⇒ 快照 ID 变化（修正须换 ID 的约定前提）。
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
        // 只有订阅占位（全 0）⇒ 过滤后无价格行，拒绝导入。
        let only_plan =
            r#"{"p-coding-plan": {"models": {"m": {"cost": {"input": 0, "output": 0}}}}}"#;
        assert!(snapshot_from_models_dev(only_plan, 0, 0).is_err());
    }
}
