//! F2 价格快照与费用估算（[价格合同](../../../docs/design/desktop-usage/pricing.md)、
//! [费用合同](../../../docs/design/desktop-usage/data-contract.md#pricing)）。
//!
//! 单位约定：价格 = 最小货币单位的百分之一 / 百万 token（i64，如 $0.075/M = 750）；
//! 计费项金额 = round_half_up(token × 价格 / 100_000_000)，i128 中间量，四舍五入到
//! 最小货币单位后累加；不用二进制浮点。
//!
//! 边界：实际渠道不明时仅展示同模型官方 API 价格参考，候选渠道/币种有歧义不套价；推理子集无独立
//! 价格行、绝不与输出重复计价；未知 token 不补零；异常 token 拒绝进入费用计算。

use crate::error::CoreError;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 仓库随版本维护的种子快照（种子导入幂等，见 [`crate::storage::pricing`]）。
pub const SEED_SNAPSHOT_JSON: &str = include_str!("../prices/seed-2026-09-25.json");
pub const SUPPLEMENT_SNAPSHOT_JSON: &str = include_str!("../prices/seed-2026-10-02.json");

/// 币种枚举（种子与本程序当前核验范围；新币种需先取得证据再扩展）。
pub const CURRENCIES: &[&str] = &["USD", "CNY"];

/// 服务档位（估算只自动匹配 standard；batch/flex/fast 行不串用）。
pub const SERVICE_TIERS: &[&str] = &["standard", "batch", "flex", "fast"];

/// 缓存写 TTL 档（分钟）。
pub const CACHE_TTL_5M_MINUTES: u32 = 5;
pub const CACHE_TTL_1H_MINUTES: u32 = 60;

/// 金额换算分母：token × 价格(百分之一最小单位/Mtok) → 最小货币单位。
const TOKEN_PRICE_DIVISOR: i128 = 100_000_000;

// ---------------------------------------------------------------------------
// 快照文件格式
// ---------------------------------------------------------------------------

/// 快照 JSON 文件（种子与用户手工导入同格式）。
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotFile {
    pub format: String,
    pub snapshot: SnapshotMetaFile,
    pub rows: Vec<PriceRowFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotMetaFile {
    pub id: String,
    /// seed / manual / community。
    pub source_type: String,
    #[serde(default)]
    pub source_urls: Vec<String>,
    /// ISO 日期（YYYY-MM-DD，UTC）。
    pub fetched_at: String,
    #[serde(default)]
    pub verified_at: Option<String>,
    #[serde(default)]
    pub verified_by: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriceRowFile {
    pub price_id: String,
    pub provider_id: String,
    pub model: String,
    pub region: String,
    pub channel: String,
    #[serde(default = "default_service_tier")]
    pub service_tier: String,
    #[serde(default)]
    pub context_threshold_tokens: Option<i64>,
    /// ISO 日期（YYYY-MM-DD，UTC 零点）。
    pub effective_from: String,
    #[serde(default)]
    pub effective_to: Option<String>,
    pub currency: String,
    #[serde(default)]
    pub input: Option<i64>,
    #[serde(default)]
    pub cache_read: Option<i64>,
    #[serde(default)]
    pub cache_write_5m: Option<i64>,
    #[serde(default)]
    pub cache_write_1h: Option<i64>,
    #[serde(default)]
    pub output: Option<i64>,
    #[serde(default)]
    pub cache_storage_hour: Option<i64>,
    /// 官方供应商按量价标记（v11；None = 按快照来源类型默认：seed/community
    /// 为 true，manual 为 false）。
    #[serde(default)]
    pub official_vendor: Option<bool>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_service_tier() -> String {
    "standard".to_string()
}

/// 解析后的快照（进入存储层前的运行时形态）。
#[derive(Debug, Clone)]
pub struct PriceSnapshot {
    pub snapshot_id: String,
    pub source_type: String,
    pub source_urls: Vec<String>,
    pub fetched_at_ms: i64,
    pub verified_at_ms: Option<i64>,
    pub content_hash: u64,
    pub license: Option<String>,
    pub verified_by: Option<String>,
    pub note: Option<String>,
    pub rows: Vec<PriceRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceRow {
    pub price_id: String,
    pub snapshot_id: String,
    pub provider_id: String,
    pub model: String,
    pub region: String,
    pub channel: String,
    pub service_tier: String,
    /// NULL 阈值归一为 0。
    pub context_threshold_tokens: i64,
    /// 半开 [from, to)；to None = 仍有效。
    pub effective_from_ms: i64,
    pub effective_to_ms: Option<i64>,
    pub input_per_mtok_hundredths: Option<i64>,
    pub cache_read_per_mtok_hundredths: Option<i64>,
    pub cache_write_5m_per_mtok_hundredths: Option<i64>,
    pub cache_write_1h_per_mtok_hundredths: Option<i64>,
    pub output_per_mtok_hundredths: Option<i64>,
    pub cache_storage_per_mtok_hour_hundredths: Option<i64>,
    pub currency: String,
    /// 官方供应商按量价标记：无精确匹配时的回退候选池（pricing.md 在线刷新设计）。
    pub official_vendor: bool,
    pub note: Option<String>,
}

/// ISO 日期（YYYY-MM-DD）→ UTC 零点毫秒。
pub(crate) fn iso_date_to_ms(text: &str, what: &str) -> Result<i64, CoreError> {
    let date = Date::strptime("%Y-%m-%d", text).map_err(|e| {
        CoreError::Validation(format!(
            "price snapshot {what} {text:?}: not an ISO date: {e}"
        ))
    })?;
    let dt = date
        .to_datetime(jiff::civil::time(0, 0, 0, 0))
        .to_zoned(jiff::tz::TimeZone::UTC)
        .map_err(|e| CoreError::Validation(format!("price snapshot {what}: {e}")))?;
    Ok(dt.timestamp().as_millisecond())
}

/// FNV-1a 内容哈希（跨版本稳定；用于同快照幂等导入判定）。
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl PriceSnapshot {
    /// 内容哈希十六进制表示（TEXT 存储形态）。
    pub fn content_hash_hex(&self) -> String {
        format!("{:016x}", self.content_hash)
    }

    /// 校验并归一化快照：格式标记、币种/档位枚举、非负价格、生效区间合法、
    /// 同键区间不重叠、行内 price_id 唯一。
    pub fn from_file(file: &SnapshotFile) -> Result<Self, CoreError> {
        if file.format != "llm-usage-price-snapshot/1" {
            return Err(CoreError::Validation(format!(
                "unknown price snapshot format {:?}",
                file.format
            )));
        }
        let meta = &file.snapshot;
        if meta.id.trim().is_empty() {
            return Err(CoreError::Validation("price snapshot id is empty".into()));
        }
        match meta.source_type.as_str() {
            "seed" | "manual" | "community" => {}
            other => {
                return Err(CoreError::Validation(format!(
                    "price snapshot source_type {other:?} not in seed/manual/community"
                )))
            }
        }
        if meta.source_type != "manual" && meta.source_urls.is_empty() {
            return Err(CoreError::Validation(format!(
                "price snapshot {} requires source URLs",
                meta.id
            )));
        }
        if file.rows.is_empty() {
            return Err(CoreError::Validation(format!(
                "price snapshot {} has no rows",
                meta.id
            )));
        }
        let fetched_at_ms = iso_date_to_ms(&meta.fetched_at, "fetched_at")?;
        let verified_at_ms = meta
            .verified_at
            .as_deref()
            .map(|v| iso_date_to_ms(v, "verified_at"))
            .transpose()?;

        let mut rows = Vec::with_capacity(file.rows.len());
        let mut seen_ids = std::collections::BTreeSet::new();
        for r in &file.rows {
            if r.price_id.trim().is_empty() || !seen_ids.insert(r.price_id.clone()) {
                return Err(CoreError::Validation(format!(
                    "price row id {:?} empty or duplicated in snapshot",
                    r.price_id
                )));
            }
            for (name, value) in [
                ("provider_id", &r.provider_id),
                ("model", &r.model),
                ("region", &r.region),
                ("channel", &r.channel),
            ] {
                if value.trim().is_empty() {
                    return Err(CoreError::Validation(format!(
                        "price row {}: {} is empty",
                        r.price_id, name
                    )));
                }
            }
            if !SERVICE_TIERS.contains(&r.service_tier.as_str()) {
                return Err(CoreError::Validation(format!(
                    "price row {}: service_tier {:?} not in {SERVICE_TIERS:?}",
                    r.price_id, r.service_tier
                )));
            }
            if !CURRENCIES.contains(&r.currency.as_str()) {
                return Err(CoreError::Validation(format!(
                    "price row {}: currency {:?} not in {CURRENCIES:?}",
                    r.price_id, r.currency
                )));
            }
            let threshold = r.context_threshold_tokens.unwrap_or(0);
            if threshold < 0 {
                return Err(CoreError::Validation(format!(
                    "price row {}: negative context threshold",
                    r.price_id
                )));
            }
            let from = iso_date_to_ms(&r.effective_from, "effective_from")?;
            let to = r
                .effective_to
                .as_deref()
                .map(|v| iso_date_to_ms(v, "effective_to"))
                .transpose()?;
            if let Some(to) = to {
                if to <= from {
                    return Err(CoreError::Validation(format!(
                        "price row {}: effective_to must be after effective_from",
                        r.price_id
                    )));
                }
            }
            for (name, value) in [
                ("input", &r.input),
                ("cache_read", &r.cache_read),
                ("cache_write_5m", &r.cache_write_5m),
                ("cache_write_1h", &r.cache_write_1h),
                ("output", &r.output),
                ("cache_storage_hour", &r.cache_storage_hour),
            ] {
                if let Some(v) = value {
                    if *v < 0 {
                        return Err(CoreError::Validation(format!(
                            "price row {}: negative {name} price",
                            r.price_id
                        )));
                    }
                }
            }
            rows.push(PriceRow {
                price_id: r.price_id.clone(),
                snapshot_id: meta.id.clone(),
                provider_id: r.provider_id.clone(),
                model: r.model.clone(),
                region: r.region.clone(),
                channel: r.channel.clone(),
                service_tier: r.service_tier.clone(),
                context_threshold_tokens: threshold,
                effective_from_ms: from,
                effective_to_ms: to,
                input_per_mtok_hundredths: r.input,
                cache_read_per_mtok_hundredths: r.cache_read,
                cache_write_5m_per_mtok_hundredths: r.cache_write_5m,
                cache_write_1h_per_mtok_hundredths: r.cache_write_1h,
                output_per_mtok_hundredths: r.output,
                cache_storage_per_mtok_hour_hundredths: r.cache_storage_hour,
                currency: r.currency.clone(),
                official_vendor: r.official_vendor.unwrap_or(meta.source_type != "manual"),
                note: r.note.clone(),
            });
        }

        // 同键区间不重叠（半开 [from, to)；to None 视为 +∞，其后不得再有同键行）。
        let mut by_key: BTreeMap<String, Vec<&PriceRow>> = BTreeMap::new();
        for row in &rows {
            let key = format!(
                "{}|{}|{}|{}|{}|{}",
                row.provider_id.to_lowercase(),
                row.model.to_lowercase(),
                row.region.to_lowercase(),
                row.channel.to_lowercase(),
                row.service_tier,
                row.context_threshold_tokens
            );
            by_key.entry(key).or_default().push(row);
        }
        for (key, group) in &by_key {
            let mut group: Vec<&&PriceRow> = group.iter().collect();
            group.sort_by_key(|r| r.effective_from_ms);
            for pair in group.windows(2) {
                let (prev, next) = (pair[0], pair[1]);
                let overlaps = match prev.effective_to_ms {
                    None => true,
                    Some(to) => next.effective_from_ms < to,
                };
                if overlaps {
                    return Err(CoreError::Validation(format!(
                        "price rows {} and {} overlap on {key}",
                        prev.price_id, next.price_id
                    )));
                }
            }
        }

        // 内容哈希：修正必须换快照 ID（A10 幂等导入的前提）。
        let mut hash_input = String::new();
        hash_input.push_str(&meta.id);
        hash_input.push_str(&meta.source_type);
        for url in &meta.source_urls {
            hash_input.push_str(url);
        }
        hash_input.push_str(&meta.fetched_at);
        for row in &rows {
            hash_input.push_str(&row.price_id);
            hash_input.push_str(&row.provider_id);
            hash_input.push_str(&row.model);
            hash_input.push_str(&row.region);
            hash_input.push_str(&row.channel);
            hash_input.push_str(&row.service_tier);
            hash_input.push_str(&row.context_threshold_tokens.to_string());
            hash_input.push_str(&row.effective_from_ms.to_string());
            if let Some(to) = row.effective_to_ms {
                hash_input.push_str(&to.to_string());
            }
            for v in [
                row.input_per_mtok_hundredths,
                row.cache_read_per_mtok_hundredths,
                row.cache_write_5m_per_mtok_hundredths,
                row.cache_write_1h_per_mtok_hundredths,
                row.output_per_mtok_hundredths,
                row.cache_storage_per_mtok_hour_hundredths,
            ] {
                hash_input.push_str(&v.map(|x| x.to_string()).unwrap_or_default());
            }
            hash_input.push_str(&row.currency);
        }
        Ok(PriceSnapshot {
            snapshot_id: meta.id.clone(),
            source_type: meta.source_type.clone(),
            source_urls: meta.source_urls.clone(),
            fetched_at_ms,
            verified_at_ms,
            content_hash: fnv1a(hash_input.as_bytes()),
            license: meta.license.clone(),
            verified_by: meta.verified_by.clone(),
            note: meta.note.clone(),
            rows,
        })
    }
}

// ---------------------------------------------------------------------------
// 估算引擎（纯函数）
// ---------------------------------------------------------------------------

/// 估算输入：单条 model_call 事件的计价相关字段。
/// 调用方负责按 quality_json 把非 known（reported/derived）字段置 None。
#[derive(Debug, Clone, Default)]
pub struct PricingEvent {
    pub provider_id: Option<String>,
    pub model_canonical: Option<String>,
    pub model_raw: Option<String>,
    pub occurred_at_ms: i64,
    pub input_uncached: Option<i64>,
    pub input_cache_read: Option<i64>,
    pub input_cache_write: Option<i64>,
    pub input_total: Option<i64>,
    pub output_total: Option<i64>,
}

/// 估算选项：精确渠道价优先；未知渠道仅允许无歧义的同型号官方参考。
#[derive(Debug, Clone, Default)]
pub struct EstimateOptions {
    /// 供应商（casefold 键）→ 用户选定的 (region, channel)。
    pub provider_channels: BTreeMap<String, (String, String)>,
    /// 供应商（casefold 键）→ 缓存写默认 TTL 档（5/60 分钟）。
    pub cache_ttl_minutes: BTreeMap<String, u32>,
}

/// 估算用价格行集合（存储层载入全部行）。
#[derive(Debug, Clone, Default)]
pub struct PriceBook {
    pub rows: Vec<PriceRow>,
}

/// 单事件估算结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventEstimate {
    /// 至少一个分量计价成功（其余分量可能未计价——见 known/priced token 与
    /// 分量 Option）。
    Priced(EventEstimateAmounts),
    /// 整条事件未计价（金额空，不写 0）。
    Unpriced(UnpricedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEstimateAmounts {
    pub currency: String,
    /// 分量金额（最小货币单位）；None = 该分量未计价（token 已知但价格缺）。
    pub input_amount_minor: Option<i64>,
    pub cache_read_amount_minor: Option<i64>,
    pub cache_write_amount_minor: Option<i64>,
    pub output_amount_minor: Option<i64>,
    /// 已计价分量金额合计（仅 Some 分量求和）。
    pub total_amount_minor: i64,
    /// 已计价 token 数（按可计价分量）。
    pub priced_tokens: i64,
    /// 已知 token 数（含未计价分量；覆盖比例 = priced/known）。
    pub known_tokens: i64,
    /// 存在未知 token 分量（输出或未缓存输入未知）：金额只能部分估算（A2）。
    pub has_unknown_components: bool,
    /// 缓存写 TTL 采用用户默认档（而非事件证据）。
    pub ttl_defaulted: bool,
    /// 无精确 provider+模型匹配时采用了官方供应商按量价行（参考估算；
    /// pricing.md 在线刷新设计）。
    pub official_fallback: bool,
    /// 参与计价的价格行（追溯）。
    pub matched_price_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnpricedReason {
    /// 来源没有报告任何可用于计价的 token 分量。
    NoKnownUsage,
    NoProvider,
    NoModel,
    /// 官方参考渠道/地区/币种仍有歧义，或未知系列缺少渠道配置。
    ChannelUnknown,
    /// 无适用价格行（含事件早于快照生效起点）。
    NoPriceRow,
    /// Separator normalization found distinct catalog IDs without an exact match.
    ModelAmbiguous,
    /// 存在多个上下文档但事件输入规模未知：不猜测档位。
    TierAmbiguous,
    /// token 异常（负值或缓存大于已知总输入）：拒绝计价。
    TokenAnomaly,
    /// 中间量溢出：拒绝该事件计价并记诊断。
    InternalOverflow,
}

impl UnpricedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            UnpricedReason::NoKnownUsage => "no_known_usage",
            UnpricedReason::NoProvider => "no_provider",
            UnpricedReason::NoModel => "no_model",
            UnpricedReason::ChannelUnknown => "channel_unknown",
            UnpricedReason::NoPriceRow => "no_price_row",
            UnpricedReason::ModelAmbiguous => "model_ambiguous",
            UnpricedReason::TierAmbiguous => "tier_ambiguous",
            UnpricedReason::TokenAnomaly => "token_anomaly",
            UnpricedReason::InternalOverflow => "internal_overflow",
        }
    }
}

/// 单分量计价：token × 价格(百分之一最小单位/Mtok) ÷ 1e8，四舍五入到最小货币单位。
fn component_amount_minor(token: i64, price_hundredths: i64) -> Result<i64, CoreError> {
    let value = (token as i128)
        .checked_mul(price_hundredths as i128)
        .ok_or(CoreError::Overflow("cost component product"))?;
    let rounded = value
        .checked_add(TOKEN_PRICE_DIVISOR / 2)
        .ok_or(CoreError::Overflow("cost component rounding"))?;
    let minor = rounded / TOKEN_PRICE_DIVISOR;
    i64::try_from(minor).map_err(|_| CoreError::Overflow("cost component amount"))
}

/// 分量槽位（金额列 + token 来源）。
#[derive(Debug, Clone, Copy)]
enum Slot {
    Input,
    CacheRead,
    Output,
}

impl Slot {
    fn set_amount(self, a: &mut EventEstimateAmounts, amount: i64) {
        match self {
            Slot::Input => a.input_amount_minor = Some(amount),
            Slot::CacheRead => a.cache_read_amount_minor = Some(amount),
            Slot::Output => a.output_amount_minor = Some(amount),
        }
    }
}

impl PriceBook {
    /// Narrow once per model, retaining all intervals, tiers and snapshot priority.
    pub fn for_model(&self, event: &PricingEvent) -> PriceBook {
        let model = crate::model_names::model_key(
            event
                .model_canonical
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .or(event.model_raw.as_deref())
                .unwrap_or_default(),
        );
        let reference = crate::model_names::reference_model_key_at(&model, event.occurred_at_ms);
        PriceBook {
            rows: self
                .rows
                .iter()
                .filter(|r| {
                    let key = crate::model_names::model_key(&r.model);
                    key == model || key == reference
                })
                .cloned()
                .collect(),
        }
    }

    /// 按发生时价计价（权威估算：区间按事件 occurred_at 匹配）。
    pub fn estimate_at_time(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
    ) -> EventEstimate {
        self.estimate(event, options, event.occurred_at_ms)
    }

    /// 按指定参考时点计价：`at_ms` 传入评估时点（now）即为"按当前价格模拟"。
    pub fn estimate(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
        at_ms: i64,
    ) -> EventEstimate {
        let provider = event
            .provider_id
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let model = event
            .model_canonical
            .as_deref()
            .filter(|m| !m.trim().is_empty())
            .or(event.model_raw.as_deref())
            .map(crate::model_names::model_key);
        let model = match model {
            Some(m) if !m.is_empty() => m,
            _ => return EventEstimate::Unpriced(UnpricedReason::NoModel),
        };
        let configured = options.provider_channels.get(&provider);
        let reference_model =
            crate::model_names::reference_model_key_at(&model, event.occurred_at_ms);

        // 异常 token：负值 / 缓存大于已知总输入（不用 max(0,…) 修饰）。
        let read = event.input_cache_read;
        let write = event.input_cache_write;
        let total_input = event.input_total;
        if [
            event.input_uncached,
            read,
            write,
            total_input,
            event.output_total,
        ]
        .iter()
        .all(Option::is_none)
        {
            return EventEstimate::Unpriced(UnpricedReason::NoKnownUsage);
        }
        if [
            event.input_uncached,
            read,
            write,
            total_input,
            event.output_total,
        ]
        .iter()
        .any(|v| v.is_some_and(|v| v < 0))
        {
            return EventEstimate::Unpriced(UnpricedReason::TokenAnomaly);
        }
        if let (Some(total), Some(r), Some(w)) = (total_input, read, write) {
            match r.checked_add(w) {
                Some(sum) if sum > total => {
                    return EventEstimate::Unpriced(UnpricedReason::TokenAnomaly)
                }
                None => return EventEstimate::Unpriced(UnpricedReason::InternalOverflow),
                _ => {}
            }
        }

        // 区间匹配：from ≤ at_ms < to（to NULL = 开放）。
        let in_range = |r: &&PriceRow| {
            (crate::model_names::model_key(&r.model) == model
                || crate::model_names::model_key(&r.model) == reference_model)
                && r.service_tier == "standard"
                && r.effective_from_ms <= at_ms
                && r.effective_to_ms.map_or(true, |to| at_ms < to)
        };
        let mut candidates: Vec<&PriceRow> = self
            .rows
            .iter()
            .filter(|r| {
                in_range(r)
                    && r.provider_id.to_lowercase() == provider
                    && configured.is_some_and(|(region, channel)| {
                        r.region.eq_ignore_ascii_case(region)
                            && r.channel.eq_ignore_ascii_case(channel)
                    })
            })
            .collect();
        if candidates
            .iter()
            .any(|r| crate::model_names::model_key(&r.model) == model)
        {
            candidates.retain(|r| crate::model_names::model_key(&r.model) == model);
        }
        // Reference pricing also works when the billing channel/provider is unknown.
        // Vendor families restrict fallback; the model release must still match exactly.
        let mut official_fallback = false;
        if candidates.is_empty() {
            let fallback: Vec<&PriceRow> = self
                .rows
                .iter()
                .filter(|r| {
                    in_range(r)
                        && crate::model_names::model_key(&r.model) == reference_model
                        && r.official_vendor
                        && (crate::model_names::official_providers(&reference_model).is_empty()
                            || crate::model_names::official_providers(&reference_model)
                                .iter()
                                .any(|p| r.provider_id.eq_ignore_ascii_case(p)))
                })
                .collect();
            if fallback.is_empty() {
                return EventEstimate::Unpriced(
                    if !crate::model_names::official_providers(&reference_model).is_empty() {
                        UnpricedReason::NoPriceRow
                    } else if provider.is_empty() {
                        UnpricedReason::NoProvider
                    } else if configured.is_none()
                        && crate::model_names::official_providers(&reference_model).is_empty()
                    {
                        UnpricedReason::ChannelUnknown
                    } else {
                        UnpricedReason::NoPriceRow
                    },
                );
            }
            let preferred: Vec<&PriceRow> = fallback
                .iter()
                .copied()
                .filter(|r| {
                    configured.is_some_and(|(region, channel)| {
                        r.region.eq_ignore_ascii_case(region)
                            && r.channel.eq_ignore_ascii_case(channel)
                    })
                })
                .collect();
            candidates = if preferred.is_empty() {
                let global: Vec<_> = fallback
                    .iter()
                    .copied()
                    .filter(|r| r.region.eq_ignore_ascii_case("global"))
                    .collect();
                if global.is_empty() {
                    fallback
                } else {
                    let api: Vec<_> = global
                        .iter()
                        .copied()
                        .filter(|r| r.channel.eq_ignore_ascii_case("api"))
                        .collect();
                    if api.is_empty() {
                        global
                    } else {
                        api
                    }
                }
            } else {
                preferred
            };
            official_fallback = true;
            // Select one vendor/channel before choosing its context tier. Mixing rows
            // across vendors/currencies can otherwise pick an unrelated high tier.
            let first = candidates[0];
            if candidates.iter().any(|r| {
                r.currency != first.currency
                    || !r.region.eq_ignore_ascii_case(&first.region)
                    || !r.channel.eq_ignore_ascii_case(&first.channel)
            }) {
                return EventEstimate::Unpriced(UnpricedReason::ChannelUnknown);
            }
            candidates.retain(|r| {
                r.provider_id == first.provider_id
                    && r.region == first.region
                    && r.channel == first.channel
                    && r.currency == first.currency
            });
        }
        // Resolve catalog spelling before context tiers: a different spelling's
        // higher tier must never override an exact ID.
        let raw_model = event
            .model_canonical
            .as_deref()
            .filter(|m| !m.trim().is_empty())
            .or(event.model_raw.as_deref())
            .unwrap_or_default()
            .trim();
        if candidates
            .iter()
            .any(|r| r.model.eq_ignore_ascii_case(raw_model))
        {
            candidates.retain(|r| r.model.eq_ignore_ascii_case(raw_model));
        } else if candidates
            .iter()
            .any(|r| r.model.eq_ignore_ascii_case(&reference_model))
        {
            candidates.retain(|r| r.model.eq_ignore_ascii_case(&reference_model));
        } else if candidates
            .iter()
            .any(|r| !r.model.eq_ignore_ascii_case(&candidates[0].model))
        {
            return EventEstimate::Unpriced(UnpricedReason::ModelAmbiguous);
        }
        // 即使只有一个高档价格行，也必须验证输入达到该行阈值。
        // 多档且输入规模未知时不能从价格簿顺序猜测档位。
        let input_size = total_input.or_else(|| {
            event
                .input_uncached?
                .checked_add(read?)?
                .checked_add(write?)
        });
        if let Some(input) = input_size {
            candidates.retain(|r| r.context_threshold_tokens <= input);
            if candidates.is_empty() {
                return EventEstimate::Unpriced(UnpricedReason::NoPriceRow);
            }
            let best = candidates
                .iter()
                .map(|r| r.context_threshold_tokens)
                .max()
                .expect("candidates non-empty");
            candidates.retain(|r| r.context_threshold_tokens == best);
        } else if candidates.iter().any(|r| r.context_threshold_tokens != 0) {
            return EventEstimate::Unpriced(UnpricedReason::TierAmbiguous);
        }
        // 相同档位跨快照重复时，候选行已按快照优先级排序。
        let estimate = self.price_with_row(event, options, candidates[0]);
        match (official_fallback, estimate) {
            (true, EventEstimate::Priced(mut amounts)) => {
                amounts.official_fallback = true;
                EventEstimate::Priced(amounts)
            }
            (_, other) => other,
        }
    }

    /// 已选定价格行后的分量计价。
    fn price_with_row(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
        row: &PriceRow,
    ) -> EventEstimate {
        // TTL 默认档按事件供应商的用户配置取（回退行 provider 与事件不同也适用）。
        let provider = event
            .provider_id
            .as_deref()
            .unwrap_or_default()
            .to_lowercase();
        // 未缓存输入：优先显式值；否则 total − read − write（三者均已知时派生）。
        let uncached = event.input_uncached.or(
            match (
                event.input_total,
                event.input_cache_read,
                event.input_cache_write,
            ) {
                (Some(t), Some(r), Some(w)) => t.checked_sub(r).and_then(|v| v.checked_sub(w)),
                _ => None,
            },
        );

        let mut amounts = EventEstimateAmounts {
            currency: row.currency.clone(),
            input_amount_minor: None,
            cache_read_amount_minor: None,
            cache_write_amount_minor: None,
            output_amount_minor: None,
            total_amount_minor: 0,
            priced_tokens: 0,
            known_tokens: 0,
            has_unknown_components: uncached.is_none() || event.output_total.is_none(),
            ttl_defaulted: false,
            official_fallback: false,
            matched_price_ids: vec![row.price_id.clone()],
        };

        let price_component = |amounts: &mut EventEstimateAmounts,
                               slot: Slot,
                               token: Option<i64>,
                               price: Option<i64>|
         -> Result<(), CoreError> {
            let tok = match token {
                Some(t) => t,
                None => return Ok(()),
            };
            amounts.known_tokens = amounts
                .known_tokens
                .checked_add(tok)
                .ok_or(CoreError::Overflow("cost known tokens"))?;
            let p = match price {
                Some(p) => p,
                None => return Ok(()), // 分量未计价：价格列缺失
            };
            let amount = component_amount_minor(tok, p)?;
            amounts.total_amount_minor = amounts
                .total_amount_minor
                .checked_add(amount)
                .ok_or(CoreError::Overflow("cost total"))?;
            amounts.priced_tokens = amounts
                .priced_tokens
                .checked_add(tok)
                .ok_or(CoreError::Overflow("cost priced tokens"))?;
            slot.set_amount(amounts, amount);
            Ok(())
        };

        let result = price_component(
            &mut amounts,
            Slot::Input,
            uncached,
            row.input_per_mtok_hundredths,
        )
        .and_then(|()| {
            price_component(
                &mut amounts,
                Slot::CacheRead,
                event.input_cache_read,
                row.cache_read_per_mtok_hundredths,
            )
        })
        .and_then(|()| {
            price_component(
                &mut amounts,
                Slot::Output,
                event.output_total,
                row.output_per_mtok_hundredths,
            )
        })
        .and_then(|()| {
            // 缓存写：TTL 档由用户默认选定（事件无 TTL 证据）；未设默认档不计价。
            let write = match event.input_cache_write {
                Some(w) => w,
                None => return Ok(()),
            };
            amounts.known_tokens = amounts
                .known_tokens
                .checked_add(write)
                .ok_or(CoreError::Overflow("cost known tokens"))?;
            // 已知零写入无需猜测 TTL，也不会产生写入费用。
            if write == 0 {
                amounts.cache_write_amount_minor = Some(0);
                return Ok(());
            }
            let ttl = options.cache_ttl_minutes.get(&provider).copied();
            let price = ttl.and_then(|minutes| match minutes {
                CACHE_TTL_1H_MINUTES => row.cache_write_1h_per_mtok_hundredths,
                _ => row.cache_write_5m_per_mtok_hundredths,
            });
            match price {
                Some(p) => {
                    let amount = component_amount_minor(write, p)?;
                    amounts.total_amount_minor = amounts
                        .total_amount_minor
                        .checked_add(amount)
                        .ok_or(CoreError::Overflow("cost total"))?;
                    amounts.priced_tokens = amounts
                        .priced_tokens
                        .checked_add(write)
                        .ok_or(CoreError::Overflow("cost priced tokens"))?;
                    amounts.cache_write_amount_minor = Some(amount);
                    amounts.ttl_defaulted = true;
                    Ok(())
                }
                None => Ok(()), // 无默认档或该档无价：写分量未计价
            }
        });
        match result {
            Ok(()) => {
                if [
                    amounts.input_amount_minor,
                    amounts.cache_read_amount_minor,
                    amounts.cache_write_amount_minor,
                    amounts.output_amount_minor,
                ]
                .iter()
                .all(Option::is_none)
                {
                    // 所有分量均未计价（价格列缺失或 token 未知）：整条未计价。
                    EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
                } else {
                    EventEstimate::Priced(amounts)
                }
            }
            Err(CoreError::Overflow(_)) => {
                EventEstimate::Unpriced(UnpricedReason::InternalOverflow)
            }
            Err(_) => EventEstimate::Unpriced(UnpricedReason::TokenAnomaly),
        }
    }
}

/// 解析快照 JSON 文本（种子或用户文件）。
pub fn parse_snapshot_json(text: &str) -> Result<PriceSnapshot, CoreError> {
    let file: SnapshotFile = serde_json::from_str(text)
        .map_err(|e| CoreError::Validation(format!("price snapshot JSON parse failed: {e}")))?;
    PriceSnapshot::from_file(&file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_json(id: &str, input: i64, read: i64, w5: Option<i64>, out: i64) -> String {
        let w5v = w5
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string());
        format!(
            r#"{{"price_id":"{id}","provider_id":"p","model":"m","region":"r","channel":"c",
                "effective_from":"2026-01-01","currency":"USD",
                "input":{input},"cache_read":{read},"cache_write_5m":{w5v},"output":{out}}}"#
        )
    }

    fn snapshot_json(rows: Vec<String>) -> String {
        format!(
            r#"{{"format":"llm-usage-price-snapshot/1",
                "snapshot":{{"id":"s1","source_type":"manual","source_urls":[],
                "fetched_at":"2026-09-25"}},
                "rows":[{}]}}"#,
            rows.join(",")
        )
    }

    #[test]
    fn parses_and_validates_seed_snapshot() {
        let snapshot = parse_snapshot_json(SEED_SNAPSHOT_JSON).expect("seed parses");
        assert!(!snapshot.rows.is_empty());
        assert!(snapshot
            .rows
            .iter()
            .all(|r| r.currency == "USD" || r.currency == "CNY"));
        // 种子内 glm-5.1 两档阈值并存且不重叠。
        let glm51: Vec<&PriceRow> = snapshot
            .rows
            .iter()
            .filter(|r| r.model == "glm-5.1")
            .collect();
        assert_eq!(glm51.len(), 2);
    }

    #[test]
    fn rejects_negative_price_and_overlap() {
        let neg = snapshot_json(vec![row_json("a", -1, 0, None, 0)]);
        assert!(parse_snapshot_json(&neg).is_err());
        // 同键两行首尾相接不重叠（from 相同则重叠）。
        let overlap = snapshot_json(vec![
            row_json("a", 10, 0, None, 0),
            row_json("b", 10, 0, None, 0),
        ]);
        assert!(parse_snapshot_json(&overlap).is_err());
    }

    #[test]
    fn amount_rounding_matches_contract() {
        // $10/M = 100000 百分之一美分；1 token × 100000 / 1e8 → 0.001 美分 → 0。
        assert_eq!(component_amount_minor(1, 100_000).unwrap(), 0);
        // 1,234,567 × 80000 / 1e8 = 987.6536 → 988（E1 输入分量）。
        assert_eq!(component_amount_minor(1_234_567, 80_000).unwrap(), 988);
        // 1,000 × 280000 / 1e8 = 2.8 → 3（E6 输出分量，四舍五入）。
        assert_eq!(component_amount_minor(1_000, 280_000).unwrap(), 3);
        // 32,768 × 80000 / 1e8 = 26.2144 → 26（E6 输入分量）。
        assert_eq!(component_amount_minor(32_768, 80_000).unwrap(), 26);
    }

    fn base_options() -> EstimateOptions {
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("p".into(), ("r".into(), "c".into()));
        options
    }

    #[test]
    fn estimate_requires_configured_channel() {
        let snapshot =
            parse_snapshot_json(&snapshot_json(vec![row_json("a", 10, 1, None, 5)])).unwrap();
        let book = PriceBook {
            rows: snapshot.rows,
        };
        let event = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_uncached: Some(100),
            output_total: Some(50),
            ..Default::default()
        };
        assert_eq!(
            book.estimate_at_time(&event, &EstimateOptions::default()),
            EventEstimate::Unpriced(UnpricedReason::ChannelUnknown)
        );
        // 配置渠道后可计价（小 token 舍入为 0 是合法金额）。
        match book.estimate_at_time(&event, &base_options()) {
            EventEstimate::Priced(a) => {
                assert_eq!(a.currency, "USD");
                assert_eq!(a.total_amount_minor, 0);
            }
            other => panic!("expected priced, got {other:?}"),
        }
    }

    #[test]
    fn token_anomaly_rejects_event() {
        let snapshot =
            parse_snapshot_json(&snapshot_json(vec![row_json("a", 10, 1, None, 5)])).unwrap();
        let book = PriceBook {
            rows: snapshot.rows,
        };
        let event = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_total: Some(100),
            input_cache_read: Some(90),
            input_cache_write: Some(20), // 110 > 100
            output_total: Some(50),
            ..Default::default()
        };
        assert_eq!(
            book.estimate_at_time(&event, &base_options()),
            EventEstimate::Unpriced(UnpricedReason::TokenAnomaly)
        );
    }

    #[test]
    fn tier_selection_requires_known_input_size() {
        // P5/P6：glm-5.1 [0,32K) ¥6/¥24 与 [32K+) ¥8/¥28（百分之一分单位）。
        let rows = [
            r#"{"price_id":"t0","provider_id":"p","model":"m","region":"r","channel":"c",
                "context_threshold_tokens":0,"effective_from":"2026-01-01","currency":"CNY",
                "input":60000,"cache_read":null,"output":240000}"#,
            r#"{"price_id":"t32k","provider_id":"p","model":"m","region":"r","channel":"c",
                "context_threshold_tokens":32768,"effective_from":"2026-01-01","currency":"CNY",
                "input":80000,"cache_read":null,"output":280000}"#,
        ]
        .join(",");
        let json = format!(
            r#"{{"format":"llm-usage-price-snapshot/1",
              "snapshot":{{"id":"s","source_type":"manual","source_urls":[],"fetched_at":"2026-09-25"}},
              "rows":[{rows}]}}"#
        );
        let snapshot = parse_snapshot_json(&json).unwrap();
        assert_eq!(snapshot.rows.len(), 2);
        let book = PriceBook {
            rows: snapshot.rows,
        };
        let base = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_total: None,
            output_total: Some(1_000),
            ..Default::default()
        };
        // 输入规模未知 → 不猜档。
        assert_eq!(
            book.estimate_at_time(&base, &base_options()),
            EventEstimate::Unpriced(UnpricedReason::TierAmbiguous)
        );
        // E6：输入 32,768 命中高档 → 26 + 3 = 29 分。
        let at_tier = PricingEvent {
            input_total: Some(32_768),
            input_uncached: Some(32_768),
            input_cache_read: Some(0),
            input_cache_write: Some(0),
            ..base.clone()
        };
        match book.estimate_at_time(&at_tier, &base_options()) {
            EventEstimate::Priced(a) => {
                assert_eq!(a.input_amount_minor, Some(26));
                assert_eq!(a.output_amount_minor, Some(3));
                assert_eq!(a.total_amount_minor, 29);
            }
            other => panic!("expected priced, got {other:?}"),
        }
        // E7：输入 32,767 命中低档 → 20 + 2 = 22 分。
        let below = PricingEvent {
            input_total: Some(32_767),
            input_uncached: Some(32_767),
            input_cache_read: Some(0),
            input_cache_write: Some(0),
            ..base
        };
        match book.estimate_at_time(&below, &base_options()) {
            EventEstimate::Priced(a) => assert_eq!(a.total_amount_minor, 22),
            other => panic!("expected priced, got {other:?}"),
        }
    }

    #[test]
    fn single_high_tier_requires_threshold_and_accepts_derived_input_size() {
        let row = r#"{"price_id":"high","provider_id":"p","model":"m","region":"r","channel":"c",
            "context_threshold_tokens":32768,"effective_from":"2026-01-01","currency":"USD",
            "input":100000,"output":500000}"#;
        let snapshot = parse_snapshot_json(&snapshot_json(vec![row.into()])).unwrap();
        let book = PriceBook {
            rows: snapshot.rows,
        };
        let base = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_uncached: Some(32_767),
            input_cache_read: Some(0),
            input_cache_write: Some(0),
            output_total: Some(0),
            ..Default::default()
        };
        assert_eq!(
            book.estimate_at_time(&base, &base_options()),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
        let unknown = PricingEvent {
            input_cache_read: None,
            ..base.clone()
        };
        assert_eq!(
            book.estimate_at_time(&unknown, &base_options()),
            EventEstimate::Unpriced(UnpricedReason::TierAmbiguous)
        );
        let at_threshold = PricingEvent {
            input_uncached: Some(32_768),
            ..base
        };
        match book.estimate_at_time(&at_threshold, &base_options()) {
            EventEstimate::Priced(amounts) => assert_eq!(amounts.matched_price_ids, ["high"]),
            other => panic!("expected high tier, got {other:?}"),
        }
    }

    #[test]
    fn cache_write_requires_ttl_default() {
        // $10/$1/$3/$50 per Mtok（百分之一美分单位）。
        let snapshot = parse_snapshot_json(&snapshot_json(vec![row_json(
            "a",
            1000,
            100,
            Some(300),
            5000,
        )]))
        .unwrap();
        let book = PriceBook {
            rows: snapshot.rows,
        };
        let event = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_uncached: Some(1_000_000),
            input_cache_write: Some(1_000_000),
            output_total: Some(1_000_000),
            ..Default::default()
        };
        // 未设默认档：写分量未计价，其余照计（10+0+50 → 部分计价）。
        match book.estimate_at_time(&event, &base_options()) {
            EventEstimate::Priced(a) => {
                assert_eq!(a.cache_write_amount_minor, None);
                assert_eq!(a.total_amount_minor, 60);
                assert!(!a.ttl_defaulted);
            }
            other => panic!("expected priced, got {other:?}"),
        }
        // 设默认 5m 档后：写分量 1M×300/1e8=3，标 defaulted。
        let mut options = base_options();
        options.cache_ttl_minutes.insert("p".into(), 5);
        match book.estimate_at_time(&event, &options) {
            EventEstimate::Priced(a) => {
                assert_eq!(a.cache_write_amount_minor, Some(3));
                assert_eq!(a.total_amount_minor, 63);
                assert!(a.ttl_defaulted);
            }
            other => panic!("expected priced, got {other:?}"),
        }
    }

    #[test]
    fn derived_uncached_input_from_totals() {
        let snapshot = parse_snapshot_json(&snapshot_json(vec![row_json(
            "a",
            1000,
            100,
            Some(300),
            5000,
        )]))
        .unwrap();
        let book = PriceBook {
            rows: snapshot.rows,
        };
        // 无显式 uncached：total 3_000_000 − read 1_000_000 − write 1_000_000 = 1M。
        let event = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_total: Some(3_000_000),
            input_cache_read: Some(1_000_000),
            input_cache_write: Some(1_000_000),
            output_total: Some(1_000_000),
            ..Default::default()
        };
        let mut options = base_options();
        options.cache_ttl_minutes.insert("p".into(), 5);
        match book.estimate_at_time(&event, &options) {
            EventEstimate::Priced(a) => {
                // 1M×1000 + 1M×100 + 1M×300 + 1M×5000 → 10+1+3+50 = 64。
                assert_eq!(a.total_amount_minor, 64);
                assert_eq!(a.known_tokens, 4_000_000);
                assert_eq!(a.priced_tokens, 4_000_000);
            }
            other => panic!("expected priced, got {other:?}"),
        }
    }

    #[test]
    fn events_before_snapshot_start_are_not_priced_at_time() {
        let snapshot =
            parse_snapshot_json(&snapshot_json(vec![row_json("a", 10, 1, None, 5)])).unwrap();
        let book = PriceBook {
            rows: snapshot.rows,
        };
        let event = PricingEvent {
            provider_id: Some("p".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_000_000_000_000, // 2001 年，早于 2026-01-01 生效起点
            input_uncached: Some(100),
            output_total: Some(50),
            ..Default::default()
        };
        assert_eq!(
            book.estimate_at_time(&event, &base_options()),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
    }

    // ------------------------------------------------------------------
    // 官方供应商回退匹配（2026-10-01 用户合同；pricing.md 在线刷新设计）
    // ------------------------------------------------------------------

    /// 带 official_vendor 标记的行 JSON（seed/community 默认 true；manual 默认 false）。
    fn row_json_official(id: &str, provider: &str, model: &str, official: Option<bool>) -> String {
        let flag = official
            .map(|v| format!(",\"official_vendor\":{v}"))
            .unwrap_or_default();
        format!(
            r#"{{"price_id":"{id}","provider_id":"{provider}","model":"{model}",
                "region":"r","channel":"c","effective_from":"2026-01-01","currency":"USD",
                "input":100000,"cache_read":10000,"cache_write_5m":null,"output":500000{flag}}}"#
        )
    }

    fn fallback_book() -> PriceBook {
        // 社区快照：官方提供商 vendorA 的 m-one（official 默认 true）。
        let community = format!(
            r#"{{"format":"llm-usage-price-snapshot/1",
                "snapshot":{{"id":"comm-1","source_type":"community",
                "source_urls":["https://models.dev/api.json"],"fetched_at":"2026-09-25"}},
                "rows":[{}]}}"#,
            row_json_official("md-vendorA-m-one", "vendorA", "m-one", None)
        );
        PriceBook {
            rows: parse_snapshot_json(&community).unwrap().rows,
        }
    }

    fn fallback_event(provider: &str) -> PricingEvent {
        PricingEvent {
            provider_id: Some(provider.into()),
            model_raw: Some("m-one".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_uncached: Some(1_000_000),
            output_total: Some(1_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn official_fallback_prices_when_exact_match_missing() {
        let book = fallback_book();
        // 事件 provider「relay-x」无精确行；回退到官方 vendorA 行并标记。
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("relay-x".into(), ("r".into(), "c".into()));
        match book.estimate_at_time(&fallback_event("relay-x"), &options) {
            EventEstimate::Priced(a) => {
                assert!(a.official_fallback);
                assert_eq!(a.total_amount_minor, 6000); // 1M×$10/M + 1M×$50/M = $60
                assert_eq!(a.matched_price_ids, ["md-vendorA-m-one"]);
            }
            other => panic!("expected fallback priced, got {other:?}"),
        }
        // 精确链命中时不回退：provider=vendorA 精确命中同一行，无回退标记。
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("vendora".into(), ("r".into(), "c".into()));
        match book.estimate_at_time(&fallback_event("vendorA"), &options) {
            EventEstimate::Priced(a) => assert!(!a.official_fallback),
            other => panic!("expected exact priced, got {other:?}"),
        }
    }

    #[test]
    fn fallback_without_configured_channel_still_requires_official_rows() {
        let book = fallback_book();
        // 无渠道配置也能展示唯一官方模型价，实付渠道仍不推断。
        assert!(
            matches!(book.estimate_at_time(&fallback_event("relay-x"), &EstimateOptions::default()),
            EventEstimate::Priced(a) if a.official_fallback && a.total_amount_minor==6000)
        );
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("relay-x".into(), ("r".into(), "c".into()));
        // 模型无官方行 ⇒ no_price_row。
        let unknown_model = PricingEvent {
            model_raw: Some("no-such-model".into()),
            ..fallback_event("relay-x")
        };
        assert_eq!(
            book.estimate_at_time(&unknown_model, &options),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
        // 手工导入行默认非官方 ⇒ 不进回退候选池。
        let manual_only = PriceBook {
            rows: parse_snapshot_json(&snapshot_json(vec![row_json_official(
                "manual-1", "vendorB", "m-two", None,
            )]))
            .unwrap()
            .rows,
        };
        let event = PricingEvent {
            model_raw: Some("m-two".into()),
            ..fallback_event("relay-x")
        };
        assert_eq!(
            manual_only.estimate_at_time(&event, &options),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
        // 手工行显式 official_vendor: true ⇒ 参与回退。
        let manual_official = PriceBook {
            rows: parse_snapshot_json(&snapshot_json(vec![row_json_official(
                "manual-2",
                "vendorB",
                "m-two",
                Some(true),
            )]))
            .unwrap()
            .rows,
        };
        match manual_official.estimate_at_time(&event, &options) {
            EventEstimate::Priced(a) => assert!(a.official_fallback),
            other => panic!("expected manual official fallback, got {other:?}"),
        }
    }

    #[test]
    fn fallback_prefers_configured_region_channel_and_keeps_tier_rules() {
        // 同一模型的两个官方行：region/channel 与配置一致者优先。
        let rows = [
            r#"{"price_id":"g","provider_id":"vendorA","model":"m","region":"global","channel":"api",
                "official_vendor":true,"effective_from":"2026-01-01","currency":"USD",
                "input":100000,"output":500000}"#,
            r#"{"price_id":"c","provider_id":"vendorA","model":"m","region":"cn","channel":"api",
                "official_vendor":true,"effective_from":"2026-01-01","currency":"CNY",
                "input":80000,"output":280000}"#,
        ]
        .join(",");
        let json = format!(
            r#"{{"format":"llm-usage-price-snapshot/1",
              "snapshot":{{"id":"s","source_type":"manual","source_urls":[],"fetched_at":"2026-09-25"}},
              "rows":[{rows}]}}"#
        );
        let book = PriceBook {
            rows: parse_snapshot_json(&json).unwrap().rows,
        };
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("relay".into(), ("cn".into(), "api".into()));
        let event = PricingEvent {
            provider_id: Some("relay".into()),
            model_raw: Some("m".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_uncached: Some(1_000_000),
            output_total: Some(1_000_000),
            ..Default::default()
        };
        match book.estimate_at_time(&event, &options) {
            EventEstimate::Priced(a) => {
                assert!(a.official_fallback);
                assert_eq!(a.currency, "CNY");
                assert_eq!(a.matched_price_ids, ["c"]);
            }
            other => panic!("expected cn-preferred fallback, got {other:?}"),
        }
        // 档位歧义在回退中同样不猜档：两个档位的官方行 + 输入规模未知。
        let tiered = [
            r#"{"price_id":"t0","provider_id":"vendorA","model":"tm","region":"global","channel":"api",
                "official_vendor":true,"context_threshold_tokens":0,"effective_from":"2026-01-01",
                "currency":"USD","input":100000,"output":500000}"#,
            r#"{"price_id":"t1","provider_id":"vendorA","model":"tm","region":"global","channel":"api",
                "official_vendor":true,"context_threshold_tokens":272000,"effective_from":"2026-01-01",
                "currency":"USD","input":200000,"output":750000}"#,
        ]
        .join(",");
        let json = format!(
            r#"{{"format":"llm-usage-price-snapshot/1",
              "snapshot":{{"id":"s2","source_type":"manual","source_urls":[],"fetched_at":"2026-09-25"}},
              "rows":[{tiered}]}}"#
        );
        let book = PriceBook {
            rows: parse_snapshot_json(&json).unwrap().rows,
        };
        let event = PricingEvent {
            provider_id: Some("relay".into()),
            model_raw: Some("tm".into()),
            occurred_at_ms: 1_800_000_000_000,
            input_uncached: None,
            output_total: Some(1_000),
            ..Default::default()
        };
        assert_eq!(
            book.estimate_at_time(&event, &options),
            EventEstimate::Unpriced(UnpricedReason::TierAmbiguous)
        );
    }
}
