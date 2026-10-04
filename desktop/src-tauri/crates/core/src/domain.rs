//! 领域类型：标准化记录、token 字段、生命周期、模型身份、时间与来源归属。
//! 语义权威来源是 docs/design/desktop-usage/data-contract.md。

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

/// 单个 token 字段允许的最大值（2^62）。超过即拒绝该记录并报错，
/// 禁止转为浮点近似；聚合时再以防溢出加法保护。
pub const MAX_TOKEN_VALUE: i64 = 1 << 62;

/// 早于该毫秒的时间戳视为不可信（2000-01-01T00:00:00Z），用于发现秒/毫秒误判。
pub const MIN_PLAUSIBLE_MS: i64 = 946_684_800_000;

/// 单个字段的数据质量。`reported` 只代表来源报告，不承诺等于最终账单。
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

/// 记录类型。统计行为见数据规范「请求、消息与累计值」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    /// 已观测到的一次模型调用/尝试；request 指标的基本单位。
    ModelCall,
    /// HTTP/WebSocket 重连或重试；独立计数，不直接增加 model_call。
    TransportAttempt,
    /// 一条 usage 记录；与调用可能一对多或多对一。
    UsageObservation,
    /// 会话/进程累计快照；按身份与重置边界求差，不能逐条求和。
    CumulativeSnapshot,
    /// 官方按日/会话等给出的汇总；保留原生范围。
    IntervalAggregate,
    /// 额度、积分、订阅窗口、余额；独立显示。
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

/// 生命周期：流式部分值 / 最终值 / 更正。
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

/// 调用类别：主调用 / 子 Agent / 辅助 / 未知。
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

/// 模型归属依据。没有不晚于调用的结构化记录时保持 unknown。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAttribution {
    /// 请求自身字段。
    RequestField,
    /// 不晚于调用的结构化模型变更/turn context。
    StructuredChange,
    /// 来源映射（如 turn_context 归属）。
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

/// 时间依据。跨午夜请求默认归到来源记录的完成时间；只有开始时间时明确标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBasis {
    SourceCompletion,
    SourceStart,
    ObservedAt,
    IntervalStart,
    /// 没有可证实的源时区/时间配置。
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

/// 版本选择依据（architecture.md 未知版本兼容约定）：
/// 已知版本按注册表映射分派；未知/缺失版本先尝试该 Agent 最新内置解析器，
/// 结果带兼容标记，兼容状态与 token 字段质量分别记录。
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

/// 本机归属依据。
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

/// 归属核验状态。无法确认归属的记录不进入总计。
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

/// token 字段集合。缺失为 None（落盘为 NULL），绝不补零。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_uncached: Option<i64>,
    pub input_cache_read: Option<i64>,
    pub input_cache_write: Option<i64>,
    pub input_total: Option<i64>,
    pub output_total: Option<i64>,
    pub output_reasoning: Option<i64>,
    pub total_tokens: Option<i64>,
    /// 来源原始总量，与规范化总量比较；不一致记诊断。
    pub source_total: Option<i64>,
}

impl TokenUsage {
    /// 逐字段校验：非负、不超过上限。超限拒绝整条记录。
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

/// 逐字段质量，与 [`TokenUsage`] 同形。unknown 字段不允许有值。
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 事件级质量分区（日聚合的低基数维度之一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityBucket {
    /// 输入与输出总量均已知。
    Complete,
    /// 部分字段已知。
    Partial,
    /// 任一字段为估算。
    Estimated,
    /// 没有任何已知 token 字段。
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

/// 费用：整数最小单位 + 币种，不用二进制浮点累加。
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

/// 进入 ingest 批次的标准化事件（model_call / transport_attempt / usage_observation）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInput {
    pub source_instance_id: String,
    /// 来源稳定记录键（response/request ID、会话 UUID+序号、文件身份+位置等）。
    pub source_record_key: String,
    pub record_kind: RecordKind,
    pub schema_version: String,
    pub parser_version: String,
    /// 版本选择依据（known_version / latest_fallback）；None 为未区分的历史数据。
    /// 兼容状态是解析依据，不是记录内容，不参与内容哈希。
    pub parse_basis: Option<VersionBasis>,
    pub origin_call_id: Option<String>,
    pub attempt_id: Option<String>,
    pub session_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub host_application: Option<String>,
    pub agent: String,
    pub call_category: CallCategory,
    /// UTC 毫秒整数。
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
    /// 源修订号；有则按修订号排序，否则按生命周期裁决。
    pub source_revision: Option<i64>,
    pub error_status: Option<String>,
    pub duration_ms: Option<i64>,
    pub ttft_ms: Option<i64>,
    pub attribution_status: AttributionStatus,
    pub exclusion_reason: Option<String>,
    pub cost: Option<CostAmount>,
}

impl EventInput {
    /// 入库前校验：token 字段非负有上限；时间戳 plausible（毫秒而非秒）。
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
