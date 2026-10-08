//! F2 price snapshots and cost estimates: see the [pricing specification](../../../docs/design/desktop-usage/pricing.md)
//! and [cost calculation rules](../../../docs/design/desktop-usage/data-contract.md#pricing).
//!
//! Rates use hundredths of a minor currency unit per million tokens (i64; $0.075/M = 750).
//! Per-event components use round_half_up(tokens * rate / 100_000_000), with i128 arithmetic.
//! Also retain exact numerators so aggregate callers can sum before rounding; no binary floats.
//!
//! Unknown billing channels permit an unambiguous official API reference for the same model.
//! Do not price reasoning twice, fill unknown tokens with zero, or price anomalous tokens.

use crate::error::CoreError;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Repository seed snapshot maintained with releases; imports skip duplicates in storage::pricing.
pub const SEED_SNAPSHOT_JSON: &str = include_str!("../prices/seed-2026-09-25.json");
pub const SUPPLEMENT_SNAPSHOT_JSON: &str = include_str!("../prices/seed-2026-10-02.json");

/// Currencies checked for the seed and current application; verify price references before adding one.
pub const CURRENCIES: &[&str] = &["USD", "CNY"];

/// Service tiers; automatic estimation selects standard without mixing batch/flex/fast rows.
pub const SERVICE_TIERS: &[&str] = &["standard", "batch", "flex", "fast"];

/// Cache-write TTL tiers in minutes.
pub const CACHE_TTL_5M_MINUTES: u32 = 5;
pub const CACHE_TTL_1H_MINUTES: u32 = 60;

/// Denominator converting tokens * hundredths of a minor unit/M tokens to minor units.
const TOKEN_PRICE_DIVISOR: i128 = 100_000_000;

// ---------------------------------------------------------------------------
// Snapshot file format.
// ---------------------------------------------------------------------------

/// Snapshot JSON format shared by seed and manually imported files.
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotFile {
    pub format: String,
    pub snapshot: SnapshotMetaFile,
    pub rows: Vec<PriceRowFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotMetaFile {
    pub id: String,
    /// Source type: seed, manual, or community.
    pub source_type: String,
    #[serde(default)]
    pub source_urls: Vec<String>,
    /// ISO date in UTC (YYYY-MM-DD).
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
    /// ISO date at UTC midnight (YYYY-MM-DD).
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
    /// Official supplier pay-as-you-go flag (v11). When absent, source type supplies the default:
    /// true for seed/community, false for manual.
    #[serde(default)]
    pub official_vendor: Option<bool>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_service_tier() -> String {
    "standard".to_string()
}

/// Parsed snapshot passed to storage.
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
    /// Normalize a NULL threshold to zero.
    pub context_threshold_tokens: i64,
    /// Half-open [from, to) interval; None for to means still effective.
    pub effective_from_ms: i64,
    pub effective_to_ms: Option<i64>,
    pub input_per_mtok_hundredths: Option<i64>,
    pub cache_read_per_mtok_hundredths: Option<i64>,
    pub cache_write_5m_per_mtok_hundredths: Option<i64>,
    pub cache_write_1h_per_mtok_hundredths: Option<i64>,
    pub output_per_mtok_hundredths: Option<i64>,
    pub cache_storage_per_mtok_hour_hundredths: Option<i64>,
    pub currency: String,
    /// Official supplier flag identifying fallback candidates when exact matching fails.
    pub official_vendor: bool,
    pub note: Option<String>,
}

/// Convert an ISO date (YYYY-MM-DD) to UTC midnight milliseconds.
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

/// Stable FNV-1a content hash for duplicate snapshot import checks.
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl PriceSnapshot {
    /// Hexadecimal content hash as stored in a TEXT column.
    pub fn content_hash_hex(&self) -> String {
        format!("{:016x}", self.content_hash)
    }

    /// Validate and normalize the format, currency/tier enums, nonnegative rates, valid intervals,
    /// nonoverlapping intervals for each key, and unique price_id values within the snapshot.
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

        // Same-key intervals are half-open; an open-ended row excludes later rows with that key.
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

        // Content changes require a new snapshot ID for A10 duplicate import checks.
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
// Pure estimation functions.
// ---------------------------------------------------------------------------

/// Price-related fields of a model_call or usage_observation event.
/// Callers map fields with quality other than reported/derived to None using quality_json.
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

/// Prefer exact channel prices; unknown channels require an unambiguous official model reference.
#[derive(Debug, Clone, Default)]
pub struct EstimateOptions {
    /// Case-folded provider keys mapped to user-selected (region, channel) pairs.
    pub provider_channels: BTreeMap<String, (String, String)>,
    /// Case-folded provider keys mapped to default cache-write TTL tiers (5/60 minutes).
    pub cache_ttl_minutes: BTreeMap<String, u32>,
}

/// All price rows loaded from storage for estimation.
#[derive(Debug, Clone, Default)]
pub struct PriceBook {
    pub rows: Vec<PriceRow>,
}

/// Estimate for one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventEstimate {
    /// At least one component was priced; other components may remain unpriced.
    /// See known/priced token counts and the component Option values.
    Priced(Box<EventEstimateAmounts>),
    /// No component was priced; leave the amount absent instead of writing zero.
    Unpriced(UnpricedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEstimateAmounts {
    /// Exact tokens * rate numerators retained until the reporting scope has been summed.
    pub component_numerators: [Option<i128>; 4],
    /// Upper bound for priced components when archived request context tiers are unknown.
    pub upper_numerators: Option<[Option<i128>; 4]>,
    pub currency: String,
    /// Component amounts in minor units; None means the component was not priced.
    pub input_amount_minor: Option<i64>,
    pub cache_read_amount_minor: Option<i64>,
    pub cache_write_amount_minor: Option<i64>,
    pub output_amount_minor: Option<i64>,
    /// Sum only the priced component amounts.
    pub total_amount_minor: i64,
    /// Tokens in components that can be priced.
    pub priced_tokens: i64,
    /// Known tokens including unpriced components; coverage is priced/known.
    pub known_tokens: i64,
    /// Unknown output or uncached input limits the estimate to known components (A2).
    pub has_unknown_components: bool,
    /// The event lacks cache-write TTL, so estimation uses the user default.
    pub ttl_defaulted: bool,
    /// An official pay-as-you-go row supplies a reference after exact provider/model matching fails;
    /// see the online price refresh specification in pricing.md.
    pub official_fallback: bool,
    /// Price model used for an explicitly authorized reference to a different model.
    pub substitute_model: Option<String>,
    /// IDs of the price rows used in this estimate.
    pub matched_price_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnpricedReason {
    /// The source reported no token components usable for pricing.
    NoKnownUsage,
    NoProvider,
    NoModel,
    /// Official reference channels/regions/currencies remain ambiguous or lack required configuration.
    ChannelUnknown,
    /// No applicable price row, including events before its effective start.
    NoPriceRow,
    /// Separator normalization found different catalog IDs without an exact match.
    ModelAmbiguous,
    /// Multiple context tiers exist but input size is unknown; leave the tier unselected.
    TierAmbiguous,
    /// Negative tokens or cache tokens exceeding known total input prevent pricing.
    TokenAnomaly,
    /// Intermediate overflow rejects pricing for this event and records a diagnostic.
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

/// Price one component: tokens * hundredths of a minor unit/M tokens / 1e8, rounded half up.
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

/// Component slot pairing an amount column with its token field.
#[derive(Debug, Clone, Copy)]
enum Slot {
    Input,
    CacheRead,
    Output,
}

impl Slot {
    fn index(self) -> usize {
        match self {
            Self::Input => 0,
            Self::CacheRead => 1,
            Self::Output => 3,
        }
    }
    fn set_amount(self, a: &mut EventEstimateAmounts, amount: i64) {
        match self {
            Slot::Input => a.input_amount_minor = Some(amount),
            Slot::CacheRead => a.cache_read_amount_minor = Some(amount),
            Slot::Output => a.output_amount_minor = Some(amount),
        }
    }
}

impl PriceBook {
    /// Filter once per model, retaining all intervals, tiers, and snapshot priorities.
    pub fn for_model(&self, event: &PricingEvent) -> PriceBook {
        let model = crate::model_names::model_key(
            event
                .model_canonical
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .or(event.model_raw.as_deref())
                .unwrap_or_default(),
        );
        let reference = crate::model_names::reference_model_key_for(
            &model,
            event.provider_id.as_deref(),
            event.occurred_at_ms,
        );
        PriceBook {
            rows: self
                .rows
                .iter()
                .filter(|r| {
                    let key = crate::model_names::model_key(&r.model);
                    key == model
                        || key == reference
                        || crate::model_names::reference_price_substitute(&reference)
                            == Some(key.as_str())
                })
                .cloned()
                .collect(),
        }
    }

    /// Estimate using the price interval containing the event occurred_at timestamp.
    pub fn estimate_at_time(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
    ) -> EventEstimate {
        self.estimate_inner(event, options, event.occurred_at_ms, false, false)
    }

    /// Estimate at at_ms; passing now selects the current price reference.
    pub fn estimate(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
        at_ms: i64,
    ) -> EventEstimate {
        self.estimate_inner(event, options, at_ms, false, true)
    }

    /// Archived sums do not identify individual request context tiers. Return component bounds
    /// within one selected tariff; do not select a tier from a whole day's input sum.
    pub fn estimate_aggregate(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
        at_ms: i64,
    ) -> EventEstimate {
        self.estimate_inner(event, options, at_ms, true, true)
    }

    fn estimate_inner(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
        at_ms: i64,
        aggregate: bool,
        allow_substitute: bool,
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
        let reference_model = crate::model_names::reference_model_key_for(
            &model,
            event.provider_id.as_deref(),
            event.occurred_at_ms,
        );

        // Reject negative tokens or cache exceeding known total input; do not replace them with zero.
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

        // Select intervals where from <= at_ms < to; NULL to means open-ended.
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
        // Unknown billing channels/providers can still show a reference without inferring actual billing.
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
                if allow_substitute {
                    if let Some(substitute) =
                        crate::model_names::reference_price_substitute(&reference_model)
                    {
                        // Reuse channel, currency, and tier checks; only official rows for this explicitly
                        // authorized substitute model may participate.
                        let substitute_book = PriceBook {
                            rows: self
                                .rows
                                .iter()
                                .filter(|r| {
                                    r.official_vendor
                                        && crate::model_names::model_key(&r.model) == substitute
                                        && crate::model_names::official_providers(substitute)
                                            .iter()
                                            .any(|p| r.provider_id.eq_ignore_ascii_case(p))
                                })
                                .cloned()
                                .collect(),
                        };
                        let mut reference_event = event.clone();
                        reference_event.model_canonical = Some(substitute.into());
                        let mut result = substitute_book.estimate_inner(
                            &reference_event,
                            options,
                            at_ms,
                            aggregate,
                            false,
                        );
                        if let EventEstimate::Priced(amounts) = &mut result {
                            amounts.substitute_model = Some(substitute.into());
                            amounts.official_fallback = true;
                        }
                        return result;
                    }
                }
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
            // Select one vendor/channel before its context tier; mixing vendors or currencies
            // could otherwise select an unrelated high tier.
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
        // Resolve catalog spelling before context tiers; a higher tier under a different
        // spelling must not override an exact model ID.
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
        // Take all tariff tiers from one prioritized snapshot; mixing a new base rate
        // with an older long-context rate would invent a tariff.
        let snapshot = candidates[0].snapshot_id.clone();
        candidates.retain(|r| r.snapshot_id == snapshot);
        if aggregate {
            if !candidates.iter().any(|r| r.context_threshold_tokens == 0) {
                return EventEstimate::Unpriced(UnpricedReason::TierAmbiguous);
            }
            let mut low = candidates[0].clone();
            let mut high = low.clone();
            macro_rules! bounds {
                ($field:ident) => {{
                    let values: Option<Vec<i64>> = candidates.iter().map(|r| r.$field).collect();
                    low.$field = values.as_ref().and_then(|v| v.iter().min().copied());
                    high.$field = values.as_ref().and_then(|v| v.iter().max().copied());
                }};
            }
            bounds!(input_per_mtok_hundredths);
            bounds!(cache_read_per_mtok_hundredths);
            bounds!(cache_write_5m_per_mtok_hundredths);
            bounds!(cache_write_1h_per_mtok_hundredths);
            bounds!(output_per_mtok_hundredths);
            return match (
                self.price_with_row(event, options, &low),
                self.price_with_row(event, options, &high),
            ) {
                (EventEstimate::Priced(mut a), EventEstimate::Priced(b)) => {
                    if a.component_numerators != b.component_numerators {
                        a.upper_numerators = Some(b.component_numerators);
                    }
                    a.official_fallback = official_fallback;
                    a.matched_price_ids = candidates.iter().map(|r| r.price_id.clone()).collect();
                    EventEstimate::Priced(a)
                }
                (other, _) => other,
            };
        }
        // Even a lone high-tier row requires input at or above its threshold.
        // Unknown input size cannot select among tiers by price-book order.
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
        // Candidates with the same tier are already ordered by snapshot priority.
        let estimate = self.price_with_row(event, options, candidates[0]);
        match (official_fallback, estimate) {
            (true, EventEstimate::Priced(mut amounts)) => {
                amounts.official_fallback = true;
                EventEstimate::Priced(amounts)
            }
            (_, other) => other,
        }
    }

    /// Price components after selecting a price row.
    fn price_with_row(
        &self,
        event: &PricingEvent,
        options: &EstimateOptions,
        row: &PriceRow,
    ) -> EventEstimate {
        // Use the event provider's configured TTL default even when the fallback provider differs.
        let provider = event
            .provider_id
            .as_deref()
            .unwrap_or_default()
            .to_lowercase();
        // Prefer explicit uncached input; otherwise derive total - read - write only if all are known.
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
            component_numerators: [None; 4],
            upper_numerators: None,
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
            substitute_model: None,
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
                None => return Ok(()), // The component is unpriced because its rate is absent.
            };
            let amount = component_amount_minor(tok, p)?;
            amounts.component_numerators[slot.index()] = Some(i128::from(tok) * i128::from(p));
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
            // Use a user-selected TTL when the event lacks one; without a default, leave writes unpriced.
            let write = match event.input_cache_write {
                Some(w) => w,
                None => return Ok(()),
            };
            amounts.known_tokens = amounts
                .known_tokens
                .checked_add(write)
                .ok_or(CoreError::Overflow("cost known tokens"))?;
            // Known zero writes require no TTL choice and incur no write amount.
            if write == 0 {
                amounts.component_numerators[2] = Some(0);
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
                    amounts.component_numerators[2] = Some(i128::from(write) * i128::from(p));
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
                None => Ok(()), // No default TTL or no rate for that tier leaves writes unpriced.
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
                    // With no priced components, return an unpriced event instead of a zero amount.
                    EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
                } else {
                    EventEstimate::Priced(Box::new(amounts))
                }
            }
            Err(CoreError::Overflow(_)) => {
                EventEstimate::Unpriced(UnpricedReason::InternalOverflow)
            }
            Err(_) => EventEstimate::Unpriced(UnpricedReason::TokenAnomaly),
        }
    }
}

/// Parse snapshot JSON from a repository seed or a user file.
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
        // The seed retains both nonoverlapping glm-5.1 context tiers.
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
        // Adjacent same-key intervals do not overlap; equal starts do.
        let overlap = snapshot_json(vec![
            row_json("a", 10, 0, None, 0),
            row_json("b", 10, 0, None, 0),
        ]);
        assert!(parse_snapshot_json(&overlap).is_err());
    }

    #[test]
    fn amount_rounding_matches_contract() {
        // $10/M = 100000 hundredths of a cent; 1 * 100000 / 1e8 = 0.001 cent, rounded to 0.
        assert_eq!(component_amount_minor(1, 100_000).unwrap(), 0);
        // E1 input: 1,234,567 * 80000 / 1e8 = 987.6536, rounded to 988.
        assert_eq!(component_amount_minor(1_234_567, 80_000).unwrap(), 988);
        // E6 output: 1,000 * 280000 / 1e8 = 2.8, rounded half up to 3.
        assert_eq!(component_amount_minor(1_000, 280_000).unwrap(), 3);
        // E6 input: 32,768 * 80000 / 1e8 = 26.2144, rounded to 26.
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
        // Configuring a channel enables pricing; a small known amount can round to zero.
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
            input_cache_write: Some(20), // Cache sum 110 exceeds total input 100.
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
        // P5/P6 rates: glm-5.1 [0,32K) at CNY 6/24 and [32K,+inf) at CNY 8/28 per M tokens.
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
        // Unknown input size leaves the context tier unselected.
        assert_eq!(
            book.estimate_at_time(&base, &base_options()),
            EventEstimate::Unpriced(UnpricedReason::TierAmbiguous)
        );
        // E6: input 32,768 selects the high tier; 26 + 3 = 29 minor units.
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
        // E7: input 32,767 selects the low tier; 20 + 2 = 22 minor units.
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
        // Synthetic rates: $0.10/$0.01/$0.03/$0.50 per M tokens (1000/100/300/5000 hundredths of a cent).
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
        // No default TTL leaves writes unpriced; input/output still total 10 + 50 = 60 cents.
        match book.estimate_at_time(&event, &base_options()) {
            EventEstimate::Priced(a) => {
                assert_eq!(a.cache_write_amount_minor, None);
                assert_eq!(a.total_amount_minor, 60);
                assert!(!a.ttl_defaulted);
            }
            other => panic!("expected priced, got {other:?}"),
        }
        // Default 5m TTL prices writes at 1M * 300 / 1e8 = 3 cents and sets defaulted.
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
        // No explicit uncached input: total 3_000_000 - read 1_000_000 - write 1_000_000 = 1M.
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
                // Divide each 1M * rate product by 1e8: 10 + 1 + 3 + 50 = 64 cents.
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
            occurred_at_ms: 1_000_000_000_000, // Year 2001 precedes the effective start 2026-01-01.
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
    // Official supplier fallback matching; see the online price refresh specification.
    // ------------------------------------------------------------------

    /// Row JSON with official_vendor; seed/community default true and manual defaults false.
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
        // Synthetic community snapshot: vendorA/m-one with the default official flag true.
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
        // relay-x has no exact price row; select the flagged vendorA reference and record fallback.
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("relay-x".into(), ("r".into(), "c".into()));
        match book.estimate_at_time(&fallback_event("relay-x"), &options) {
            EventEstimate::Priced(a) => {
                assert!(a.official_fallback);
                assert_eq!(a.total_amount_minor, 6000); // 1M * $10/M + 1M * $50/M = $60.
                assert_eq!(a.matched_price_ids, ["md-vendorA-m-one"]);
            }
            other => panic!("expected fallback priced, got {other:?}"),
        }
        // Exact provider vendorA matches the same row without a fallback flag.
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
        // An unambiguous official reference needs no channel configuration or actual billing inference.
        assert!(
            matches!(book.estimate_at_time(&fallback_event("relay-x"), &EstimateOptions::default()),
            EventEstimate::Priced(a) if a.official_fallback && a.total_amount_minor==6000)
        );
        let mut options = EstimateOptions::default();
        options
            .provider_channels
            .insert("relay-x".into(), ("r".into(), "c".into()));
        // A model with no official row returns no_price_row.
        let unknown_model = PricingEvent {
            model_raw: Some("no-such-model".into()),
            ..fallback_event("relay-x")
        };
        assert_eq!(
            book.estimate_at_time(&unknown_model, &options),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
        // Manual rows default to nonofficial and are excluded from fallback candidates.
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
        // Explicit official_vendor: true allows a manual row to participate in fallback.
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
        // Prefer the official row with region/channel matching the configured values.
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
        // Fallback also leaves unknown request tiers unselected when two context tiers exist.
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
