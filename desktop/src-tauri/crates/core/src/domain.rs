//! Domain types: normalized records, tokens, lifecycle, model identity, time and source ownership.
//! Field meanings follow docs/design/desktop-usage/data-contract.md.

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

/// Maximum value for one token field (2^62). Reject larger values with an error;
/// preserve integer precision and check aggregate additions for overflow.
pub const MAX_TOKEN_VALUE: i64 = 1 << 62;

/// Reject timestamps before 2000-01-01T00:00:00Z to catch mistaken seconds/milliseconds.
pub const MIN_PLAUSIBLE_MS: i64 = 946_684_800_000;

/// Field quality: reported means the source reports a value, without establishing the final bill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldQuality {
    Reported,
    Derived,
    Estimated,
    Unknown,
}

impl FieldQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldQuality::Reported => "reported",
            FieldQuality::Derived => "derived",
            FieldQuality::Estimated => "estimated",
            FieldQuality::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "reported" => Ok(FieldQuality::Reported),
            "derived" => Ok(FieldQuality::Derived),
            "estimated" => Ok(FieldQuality::Estimated),
            "unknown" => Ok(FieldQuality::Unknown),
            other => Err(CoreError::Validation(format!(
                "unknown field quality: {other}"
            ))),
        }
    }
}

/// Record kinds; statistical behavior follows the data rules for requests, messages and cumulative values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    /// One observed model call/attempt; the basic request-count unit.
    ModelCall,
    /// HTTP/WebSocket reconnect or retry; count separately without increasing model_call directly.
    TransportAttempt,
    /// A usage observation; its relationship to calls may be one-to-many or many-to-one.
    UsageObservation,
    /// Session/process cumulative snapshot; take differences by identity/reset boundaries rather than sum rows.
    CumulativeSnapshot,
    /// Native day/session or other aggregate, retaining its original scope.
    IntervalAggregate,
    /// Quota, credits, subscription window or balance, displayed separately.
    QuotaSnapshot,
}

impl RecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RecordKind::ModelCall => "model_call",
            RecordKind::TransportAttempt => "transport_attempt",
            RecordKind::UsageObservation => "usage_observation",
            RecordKind::CumulativeSnapshot => "cumulative_snapshot",
            RecordKind::IntervalAggregate => "interval_aggregate",
            RecordKind::QuotaSnapshot => "quota_snapshot",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "model_call" => Ok(RecordKind::ModelCall),
            "transport_attempt" => Ok(RecordKind::TransportAttempt),
            "usage_observation" => Ok(RecordKind::UsageObservation),
            "cumulative_snapshot" => Ok(RecordKind::CumulativeSnapshot),
            "interval_aggregate" => Ok(RecordKind::IntervalAggregate),
            "quota_snapshot" => Ok(RecordKind::QuotaSnapshot),
            other => Err(CoreError::Validation(format!(
                "unknown record kind: {other}"
            ))),
        }
    }
}

/// Streaming partial values, final values or corrections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Partial = 0,
    Final = 1,
    Corrected = 2,
}

impl Lifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Lifecycle::Partial => "partial",
            Lifecycle::Final => "final",
            Lifecycle::Corrected => "corrected",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "partial" => Ok(Lifecycle::Partial),
            "final" => Ok(Lifecycle::Final),
            "corrected" => Ok(Lifecycle::Corrected),
            other => Err(CoreError::Validation(format!("unknown lifecycle: {other}"))),
        }
    }
}

/// Primary, subagent, auxiliary or unknown call category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallCategory {
    Primary,
    SubAgent,
    Auxiliary,
    Unknown,
}

impl CallCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            CallCategory::Primary => "primary",
            CallCategory::SubAgent => "sub_agent",
            CallCategory::Auxiliary => "auxiliary",
            CallCategory::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "primary" => Ok(CallCategory::Primary),
            "sub_agent" => Ok(CallCategory::SubAgent),
            "auxiliary" => Ok(CallCategory::Auxiliary),
            "unknown" => Ok(CallCategory::Unknown),
            other => Err(CoreError::Validation(format!(
                "unknown call category: {other}"
            ))),
        }
    }
}

/// Model attribution; keep unknown without structured information available by the call time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAttribution {
    /// Model field on the request itself.
    RequestField,
    /// Structured model change/turn context available by the call time.
    StructuredChange,
    /// Source mapping, such as turn_context ownership.
    ProviderMapping,
    Unknown,
}

impl ModelAttribution {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelAttribution::RequestField => "request_field",
            ModelAttribution::StructuredChange => "structured_change",
            ModelAttribution::ProviderMapping => "provider_mapping",
            ModelAttribution::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "request_field" => Ok(ModelAttribution::RequestField),
            "structured_change" => Ok(ModelAttribution::StructuredChange),
            "provider_mapping" => Ok(ModelAttribution::ProviderMapping),
            "unknown" => Ok(ModelAttribution::Unknown),
            other => Err(CoreError::Validation(format!(
                "unknown model attribution: {other}"
            ))),
        }
    }
}

/// Time basis: assign midnight-crossing calls by native completion; identify start-only timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBasis {
    SourceCompletion,
    SourceStart,
    ObservedAt,
    IntervalStart,
    /// Native timezone/time configuration has not been verified.
    Uncertain,
}

impl TimeBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            TimeBasis::SourceCompletion => "source_completion",
            TimeBasis::SourceStart => "source_start",
            TimeBasis::ObservedAt => "observed_at",
            TimeBasis::IntervalStart => "interval_start",
            TimeBasis::Uncertain => "uncertain",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "source_completion" => Ok(TimeBasis::SourceCompletion),
            "source_start" => Ok(TimeBasis::SourceStart),
            "observed_at" => Ok(TimeBasis::ObservedAt),
            "interval_start" => Ok(TimeBasis::IntervalStart),
            "uncertain" => Ok(TimeBasis::Uncertain),
            other => Err(CoreError::Validation(format!(
                "unknown time basis: {other}"
            ))),
        }
    }
}

/// Version selection follows architecture.md compatibility rules:
/// dispatch known versions through the registry; unknown/missing versions try the latest built-in reader
/// for that Agent. Record compatibility separately from token-field quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionBasis {
    KnownVersion,
    LatestFallback,
}

impl VersionBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionBasis::KnownVersion => "known_version",
            VersionBasis::LatestFallback => "latest_fallback",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "known_version" => Ok(VersionBasis::KnownVersion),
            "latest_fallback" => Ok(VersionBasis::LatestFallback),
            other => Err(CoreError::Validation(format!(
                "unknown version basis: {other}"
            ))),
        }
    }
}

/// Basis for identifying this device as the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalityBasis {
    LocalFilesystem,
    WslInstance,
    Container,
    RemoteSync,
    CloudReport,
    Unknown,
}

impl LocalityBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            LocalityBasis::LocalFilesystem => "local_filesystem",
            LocalityBasis::WslInstance => "wsl_instance",
            LocalityBasis::Container => "container",
            LocalityBasis::RemoteSync => "remote_sync",
            LocalityBasis::CloudReport => "cloud_report",
            LocalityBasis::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "local_filesystem" => Ok(LocalityBasis::LocalFilesystem),
            "wsl_instance" => Ok(LocalityBasis::WslInstance),
            "container" => Ok(LocalityBasis::Container),
            "remote_sync" => Ok(LocalityBasis::RemoteSync),
            "cloud_report" => Ok(LocalityBasis::CloudReport),
            "unknown" => Ok(LocalityBasis::Unknown),
            other => Err(CoreError::Validation(format!(
                "unknown locality basis: {other}"
            ))),
        }
    }
}

/// Ownership verification; records with unverified ownership do not enter totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionStatus {
    Verified,
    Pending,
    Excluded,
}

impl AttributionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AttributionStatus::Verified => "verified",
            AttributionStatus::Pending => "pending",
            AttributionStatus::Excluded => "excluded",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "verified" => Ok(AttributionStatus::Verified),
            "pending" => Ok(AttributionStatus::Pending),
            "excluded" => Ok(AttributionStatus::Excluded),
            other => Err(CoreError::Validation(format!(
                "unknown attribution status: {other}"
            ))),
        }
    }
}

/// Token fields; missing values are None (stored as NULL), without replacement zeros.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_uncached: Option<i64>,
    pub input_cache_read: Option<i64>,
    pub input_cache_write: Option<i64>,
    pub input_total: Option<i64>,
    pub output_total: Option<i64>,
    pub output_reasoning: Option<i64>,
    pub total_tokens: Option<i64>,
    /// Original source total, compared with normalized totals; disagreements produce diagnostics.
    pub source_total: Option<i64>,
}

impl TokenUsage {
    /// Validate every field as nonnegative and within the limit; reject an out-of-range record.
    pub fn validate(&self) -> Result<(), CoreError> {
        let fields = [
            ("input_uncached", self.input_uncached),
            ("input_cache_read", self.input_cache_read),
            ("input_cache_write", self.input_cache_write),
            ("input_total", self.input_total),
            ("output_total", self.output_total),
            ("output_reasoning", self.output_reasoning),
            ("total_tokens", self.total_tokens),
            ("source_total", self.source_total),
        ];
        for (name, value) in fields {
            if let Some(v) = value {
                if v < 0 {
                    return Err(CoreError::Validation(format!(
                        "token field {name} is negative: {v}"
                    )));
                }
                if v > MAX_TOKEN_VALUE {
                    return Err(CoreError::Validation(format!(
                        "token field {name} exceeds max {MAX_TOKEN_VALUE}: {v}"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Per-field quality matching TokenUsage; unknown fields must have no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenQuality {
    pub input_uncached: FieldQuality,
    pub input_cache_read: FieldQuality,
    pub input_cache_write: FieldQuality,
    pub input_total: FieldQuality,
    pub output_total: FieldQuality,
    pub output_reasoning: FieldQuality,
    pub total_tokens: FieldQuality,
    pub source_total: FieldQuality,
}

impl Default for TokenQuality {
    fn default() -> Self {
        Self {
            input_uncached: FieldQuality::Unknown,
            input_cache_read: FieldQuality::Unknown,
            input_cache_write: FieldQuality::Unknown,
            input_total: FieldQuality::Unknown,
            output_total: FieldQuality::Unknown,
            output_reasoning: FieldQuality::Unknown,
            total_tokens: FieldQuality::Unknown,
            source_total: FieldQuality::Unknown,
        }
    }
}

impl TokenQuality {
    pub fn validate(&self, usage: &TokenUsage) -> Result<(), CoreError> {
        for (name, value, quality) in [
            ("input_uncached", usage.input_uncached, self.input_uncached),
            (
                "input_cache_read",
                usage.input_cache_read,
                self.input_cache_read,
            ),
            (
                "input_cache_write",
                usage.input_cache_write,
                self.input_cache_write,
            ),
            ("input_total", usage.input_total, self.input_total),
            ("output_total", usage.output_total, self.output_total),
            (
                "output_reasoning",
                usage.output_reasoning,
                self.output_reasoning,
            ),
            ("total_tokens", usage.total_tokens, self.total_tokens),
            ("source_total", usage.source_total, self.source_total),
        ] {
            if value.is_none() != (quality == FieldQuality::Unknown) {
                return Err(CoreError::Validation(format!(
                    "token field {name} value and quality disagree"
                )));
            }
        }
        Ok(())
    }

    pub fn any_estimated(&self) -> bool {
        [
            self.input_uncached,
            self.input_cache_read,
            self.input_cache_write,
            self.input_total,
            self.output_total,
            self.output_reasoning,
            self.total_tokens,
            self.source_total,
        ]
        .contains(&FieldQuality::Estimated)
    }
}

/// Event quality category, used when grouping daily aggregates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityBucket {
    /// Both input and output totals are known.
    Complete,
    /// Some token fields are known.
    Partial,
    /// At least one field is estimated.
    Estimated,
    /// No token field is known.
    Unknown,
}

impl QualityBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            QualityBucket::Complete => "complete",
            QualityBucket::Partial => "partial",
            QualityBucket::Estimated => "estimated",
            QualityBucket::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "complete" => Ok(QualityBucket::Complete),
            "partial" => Ok(QualityBucket::Partial),
            "estimated" => Ok(QualityBucket::Estimated),
            "unknown" => Ok(QualityBucket::Unknown),
            other => Err(CoreError::Validation(format!(
                "unknown quality bucket: {other}"
            ))),
        }
    }

    pub fn of(usage: &TokenUsage, quality: &TokenQuality) -> Self {
        if quality.any_estimated() {
            return QualityBucket::Estimated;
        }
        let any_known = [
            usage.input_uncached,
            usage.input_cache_read,
            usage.input_cache_write,
            usage.input_total,
            usage.output_total,
            usage.output_reasoning,
            usage.total_tokens,
            usage.source_total,
        ]
        .iter()
        .any(Option::is_some);
        if !any_known {
            QualityBucket::Unknown
        } else if usage.input_total.is_some() && usage.output_total.is_some() {
            QualityBucket::Complete
        } else {
            QualityBucket::Partial
        }
    }
}

/// Cost in integer smallest units with a currency; avoid floating-point accumulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostAmount {
    pub amount_minor: i64,
    pub currency: String,
    pub kind: CostKind,
    pub price_version: Option<String>,
    pub billing_scope: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostKind {
    Reported,
    Estimated,
}

impl CostKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CostKind::Reported => "reported",
            CostKind::Estimated => "estimated",
        }
    }
}

/// Normalized ingest event: model_call, transport_attempt or usage_observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInput {
    pub source_instance_id: String,
    /// Stable native key: response/request ID, session UUID plus sequence, or file identity plus position.
    pub source_record_key: String,
    pub record_kind: RecordKind,
    pub schema_version: String,
    pub parser_version: String,
    /// Reader selection: known_version/latest_fallback; None identifies undifferentiated historical data.
    /// Compatibility describes parsing, rather than event content, and is excluded from the content hash.
    pub parse_basis: Option<VersionBasis>,
    pub origin_call_id: Option<String>,
    pub attempt_id: Option<String>,
    pub session_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub host_application: Option<String>,
    pub agent: String,
    pub call_category: CallCategory,
    /// Integer UTC milliseconds.
    pub occurred_at_ms: i64,
    pub observed_at_ms: Option<i64>,
    pub source_time: Option<String>,
    pub time_basis: TimeBasis,
    pub interval_start_ms: Option<i64>,
    pub interval_end_ms: Option<i64>,
    pub provider_id: Option<String>,
    pub model_raw: Option<String>,
    pub model_canonical: Option<String>,
    pub model_attribution: ModelAttribution,
    pub usage: TokenUsage,
    pub quality: TokenQuality,
    pub lifecycle: Lifecycle,
    /// Prefer source revision ordering when available; otherwise resolve by lifecycle.
    pub source_revision: Option<i64>,
    pub error_status: Option<String>,
    pub duration_ms: Option<i64>,
    pub ttft_ms: Option<i64>,
    pub attribution_status: AttributionStatus,
    pub exclusion_reason: Option<String>,
    pub cost: Option<CostAmount>,
}

impl EventInput {
    /// Before storage, validate bounded nonnegative tokens and plausible millisecond timestamps.
    pub fn validate(&self) -> Result<(), CoreError> {
        self.usage.validate()?;
        self.quality.validate(&self.usage)?;
        if !matches!(
            self.record_kind,
            RecordKind::ModelCall | RecordKind::TransportAttempt | RecordKind::UsageObservation
        ) {
            return Err(CoreError::Validation(
                "aggregate and quota records require their dedicated storage API".into(),
            ));
        }
        for (name, ts) in [
            ("occurred_at_ms", Some(self.occurred_at_ms)),
            ("observed_at_ms", self.observed_at_ms),
            ("interval_start_ms", self.interval_start_ms),
            ("interval_end_ms", self.interval_end_ms),
        ] {
            if let Some(v) = ts {
                if v < MIN_PLAUSIBLE_MS {
                    return Err(CoreError::Validation(format!(
                        "{name}={v} is before 2000-01-01; likely a seconds/milliseconds mixup"
                    )));
                }
            }
        }
        if let (Some(start), Some(end)) = (self.interval_start_ms, self.interval_end_ms) {
            if end < start {
                return Err(CoreError::Validation(format!(
                    "interval end {end} before start {start}"
                )));
            }
        }
        Ok(())
    }
}
