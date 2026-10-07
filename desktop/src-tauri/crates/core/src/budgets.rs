//! Opt-in local reminders. Unknown observations never become zero.
use crate::calendar::{Calendar, WeekStart};
use crate::error::CoreError;
use crate::pricing::EstimateOptions;
use crate::query::{query_summary, Filters, Granularity, SummaryRequest};
use crate::storage::{
    pricing::{CostFilters, CostSummaryRequest},
    Storage,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMetric {
    TotalTokens,
    EstimatedCost,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Day,
    Month,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BudgetSettings {
    pub enabled: bool,
    pub metric: BudgetMetric,
    pub period: BudgetPeriod,
    /// Integer tokens or micro currency units, serialized exactly for IPC.
    pub threshold: String,
    pub currency: String,
}
impl Default for BudgetSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            metric: BudgetMetric::TotalTokens,
            period: BudgetPeriod::Month,
            threshold: "1000000".into(),
            currency: "USD".into(),
        }
    }
}
impl BudgetSettings {
    pub fn validate(&self) -> Result<i64, CoreError> {
        let threshold = self
            .threshold
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                CoreError::Validation("budget threshold must be a positive integer".into())
            })?;
        if self.currency.len() != 3 || !self.currency.bytes().all(|c| c.is_ascii_uppercase()) {
            return Err(CoreError::Validation(
                "budget currency must have three uppercase letters".into(),
            ));
        }
        Ok(threshold)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    #[serde(skip)]
    evaluated_revision: i64,
    #[serde(skip)]
    evaluated_data_version: i64,
    pub enabled: bool,
    pub current: Option<String>,
    pub threshold: String,
    pub metric: BudgetMetric,
    pub currency: String,
    pub first_day: String,
    pub last_day: String,
    pub coverage_limited: bool,
    pub exceeded: bool,
    pub newly_triggered: bool,
}

pub struct BudgetRequest<'a> {
    pub settings: &'a BudgetSettings,
    pub timezone: &'a str,
    pub user_id: &'a str,
    pub instances: Vec<String>,
    pub now_ms: i64,
    pub pricing_enabled: bool,
    pub price_options: EstimateOptions,
}

pub fn evaluate(storage: &Storage, request: &BudgetRequest<'_>) -> Result<BudgetStatus, CoreError> {
    let cfg = request.settings;
    let threshold = cfg.validate()?;
    let calendar = Calendar::new(request.timezone)?;
    let today = calendar.local_day_of(request.now_ms)?;
    let first = match cfg.period {
        BudgetPeriod::Day => today,
        BudgetPeriod::Month => today
            .with()
            .day(1)
            .build()
            .map_err(|e| CoreError::Calendar(e.to_string()))?,
    };
    let mut result = BudgetStatus {
        evaluated_revision: storage.data_revision()?,
        evaluated_data_version: storage
            .conn()
            .pragma_query_value(None, "data_version", |r| r.get(0))?,
        enabled: cfg.enabled,
        current: None,
        threshold: threshold.to_string(),
        metric: cfg.metric.clone(),
        currency: cfg.currency.clone(),
        first_day: first.to_string(),
        last_day: today.to_string(),
        coverage_limited: false,
        exceeded: false,
        newly_triggered: false,
    };
    if !cfg.enabled {
        return Ok(result);
    }
    let summary = query_summary(
        storage,
        &SummaryRequest {
            timezone: request.timezone.into(),
            week_start: WeekStart::Monday,
            first_day: first,
            last_day: today,
            granularity: Granularity::Day,
            filters: Filters {
                instances: Some(request.instances.clone()),
                ..Default::default()
            },
            today,
            retention_cutoff: None,
        },
    )?;
    let totals = &summary.totals;
    result.coverage_limited = request.instances.is_empty()
        || totals.total_unknown_count > 0
        || totals.conflict_count > 0
        || summary.periods.iter().any(|p| p.partial_history)
        || summary.excluded_event_count > 0;
    for instance in &request.instances {
        let healthy:bool=storage.conn().query_row("SELECT EXISTS(SELECT 1 FROM source_instances WHERE instance_id=?1 AND health='ok' AND attribution_status='verified')",[instance],|r|r.get(0))?;
        result.coverage_limited |= !healthy;
        let unknown:bool=storage.conn().query_row("SELECT EXISTS(SELECT 1 FROM daily_usage WHERE instance_id=?1 AND tz_version=?2 AND local_day>=?3 AND local_day<=?4 AND quality_bucket='unknown' AND event_count>0)",params![instance,request.timezone,first.to_string(),today.to_string()],|r|r.get(0))?;
        result.coverage_limited |= unknown;
    }
    let current = match cfg.metric {
        BudgetMetric::TotalTokens => totals.total_tokens_known,
        BudgetMetric::EstimatedCost if request.pricing_enabled => {
            let costs = storage.cost_summary(&CostSummaryRequest {
                timezone: request.timezone.into(),
                first_day: first.to_string(),
                last_day: today.to_string(),
                filters: CostFilters {
                    instances: Some(request.instances.clone()),
                    ..Default::default()
                },
                now_ms: request.now_ms,
                options: request.price_options.clone(),
            })?;
            result.coverage_limited |=
                costs.at_time.detail_limited || !costs.at_time.unpriced_reasons.is_empty();
            let row = costs
                .at_time
                .rows
                .iter()
                .find(|r| r.currency == cfg.currency);
            if let Some(row) = row {
                result.coverage_limited |= row.partial_event_count > 0
                    || row.fallback_event_count > 0
                    || row.unpriced_event_count > 0;
            }
            row.map(|r| r.total_amount_minor)
        }
        BudgetMetric::EstimatedCost => {
            result.coverage_limited = true;
            None
        }
    };
    result.current = current.map(|v| v.to_string());
    result.coverage_limited |= current.is_none();
    result.exceeded = current.is_some_and(|n| n >= threshold);
    if storage.data_revision()? != result.evaluated_revision
        || storage
            .conn()
            .pragma_query_value(None, "data_version", |r| r.get::<_, i64>(0))?
            != result.evaluated_data_version
    {
        return Err(CoreError::Validation(
            "budget data changed during query; retry".into(),
        ));
    }
    Ok(result)
}

/// Claim only after a visible GUI requests the reminder; headless scans do not consume it.
pub fn claim(storage: &Storage, request: &BudgetRequest<'_>) -> Result<BudgetStatus, CoreError> {
    let mut status = evaluate(storage, request)?;
    if !status.exceeded {
        return Ok(status);
    }
    let cfg = request.settings;
    let identity = serde_json::to_string(&(
        request.user_id,
        request.timezone,
        &cfg.period,
        &status.first_day,
        &cfg.metric,
        &cfg.currency,
        &status.threshold,
    ))?;
    // Keep the complete identity: a hash collision must not suppress a reminder.
    let key = format!("budget_notice:{identity}");
    let tx = storage.conn().unchecked_transaction()?;
    if storage.data_revision()? != status.evaluated_revision
        || storage
            .conn()
            .pragma_query_value(None, "data_version", |r| r.get::<_, i64>(0))?
            != status.evaluated_data_version
    {
        return Err(CoreError::Validation(
            "budget data changed before reminder; retry".into(),
        ));
    }
    status.newly_triggered=tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,'shown',1,?2) ON CONFLICT(key) DO NOTHING",params![key,request.now_ms])?>0;
    tx.commit()?;
    Ok(status)
}
