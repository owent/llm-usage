//! Qoder CN VS Code 2.6.11 native chat metadata; no persisted token counts.
use crate::{adapters::framework::*, domain::*, error::CoreError, ingest::DiagnosticInput};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

pub const FORMAT: &str = "qoder-cn-vscode-chat-db";
pub const PARSER: &str = "qoder-cn-vscode-db-1";

fn open(path: &Path) -> Result<Connection, CoreError> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_millis(150))?;
    crate::adapters::run_policy::install_sqlite_control(&mut conn)?;
    Ok(conn)
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let conn = open(path)?;
    for (table, required) in [
        ("chat_session", &["session_id", "version"][..]),
        (
            "chat_record",
            &[
                "request_id",
                "session_id",
                "gmt_create",
                "gmt_modified",
                "extra",
                "chat_task",
                "mode",
                "session_type",
            ][..],
        ),
        ("chat_message", &["session_id", "request_id", "role"][..]),
    ] {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if required.iter().any(|key| !columns.iter().any(|c| c == key)) {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "missing Qoder CN chat database columns".into(),
            });
        }
    }
    Ok(DetectOutcome::Supported {
        format: FORMAT.into(),
        format_version: None,
        basis: VersionBasis::LatestFallback,
    })
}

pub fn native(path: &Path) -> bool {
    is_sqlite(path) && matches!(detect(path), Ok(DetectOutcome::Supported { .. }))
}

pub fn is_sqlite(path: &Path) -> bool {
    crate::adapters::framework::read_detect_head(path, 16)
        .ok()
        .flatten()
        .is_some_and(|head| head.starts_with(b"SQLite format 3\0"))
}

#[derive(Default, Serialize, Deserialize)]
struct Cursor {
    parser: String,
    rowid: i64,
    invalid: bool,
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut conn = open(&target.path)?;
    let tx = conn.transaction()?;
    let mut cursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value::<Cursor>(v.clone()).ok())
        .filter(|c| c.parser == PARSER && !target.rescan)
        .unwrap_or_default();
    let mut out = ScanOutcome {
        status: ScanStatus::Complete,
        cursor: None,
        parse_context: None,
        events: vec![],
        aggregates: vec![],
        diagnostics: vec![],
        lines_read: 0,
        records_seen: 0,
        reconciliations: vec![],
        health: "active".into(),
    };
    let limit = limits.jsonl.max_lines.unwrap_or(10_000).clamp(1, 10_000);
    let mut stmt = tx.prepare("SELECT r.rowid,r.request_id,r.session_id,r.gmt_create,r.gmt_modified,s.version,
        CASE WHEN length(r.extra)<=1048576 THEN CASE WHEN json_valid(r.extra) THEN json_extract(r.extra,'$.modelConfig.key') END END,
        r.extra IS NULL OR CASE WHEN length(r.extra)<=1048576 THEN json_valid(r.extra) ELSE 0 END,r.chat_task,r.mode,r.session_type,
        EXISTS(SELECT 1 FROM chat_message m WHERE m.request_id=r.request_id AND m.session_id=r.session_id AND m.role='assistant')
        FROM chat_record r LEFT JOIN chat_session s ON s.session_id=r.session_id
        WHERE r.rowid>?1 ORDER BY r.rowid LIMIT ?2")?;
    let mut rows = stmt.query(rusqlite::params![cursor.rowid, limit as i64])?;
    while let Some(row) = rows.next()? {
        crate::adapters::run_policy::check()?;
        cursor.rowid = row.get(0)?;
        out.records_seen += 1;
        let parsed = (|| -> rusqlite::Result<Option<EventInput>> {
            let request: String = row.get(1)?;
            let session: String = row.get(2)?;
            let created: i64 = row.get(3)?;
            let modified: i64 = row.get(4)?;
            let version: String = row.get(5)?;
            let model: Option<String> = row.get(6)?;
            let valid_extra: bool = row.get(7)?;
            let task: String = row.get(8)?;
            let mode: String = row.get(9)?;
            let surface: String = row.get(10)?;
            let assistant: bool = row.get(11)?;
            if request.trim().is_empty()
                || session.trim().is_empty()
                || version.trim().is_empty()
                || !(MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(&created)
                || modified < created
                || !valid_extra
            {
                return Err(rusqlite::Error::InvalidQuery);
            }
            if !assistant || task != "FREE_INPUT" || mode != "chat" || surface != "assistant" {
                return Ok(None);
            }
            Ok(Some(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: format!(
                    "cn-chat:{}",
                    serde_json::to_string(&[&session, &request]).unwrap()
                ),
                record_kind: RecordKind::UsageObservation,
                schema_version: format!("qoder-cn-chat-db/{version}"),
                parser_version: PARSER.into(),
                parse_basis: Some(VersionBasis::LatestFallback),
                origin_call_id: None,
                attempt_id: None,
                session_id: Some(session),
                parent_session_id: None,
                host_application: Some("vscode".into()),
                agent: "qoder".into(),
                call_category: CallCategory::Unknown,
                occurred_at_ms: created,
                observed_at_ms: Some(now_ms),
                source_time: None,
                time_basis: TimeBasis::ObservedAt,
                interval_start_ms: None,
                interval_end_ms: None,
                provider_id: None,
                model_raw: model.filter(|s| !s.trim().is_empty()),
                model_canonical: None,
                model_attribution: ModelAttribution::RequestField,
                usage: TokenUsage::default(),
                quality: TokenQuality::default(),
                lifecycle: Lifecycle::Final,
                source_revision: Some(modified),
                error_status: None,
                duration_ms: None,
                ttft_ms: None,
                attribution_status: AttributionStatus::Verified,
                exclusion_reason: None,
                cost: None,
            }))
        })();
        match parsed {
            Ok(Some(event)) => out.events.push(event),
            Ok(None) => {}
            Err(_) => {
                cursor.invalid = true;
                out.diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "qoder_cn_row_invalid".into(),
                    field: None,
                    position: Some(format!("rowid {}", cursor.rowid)),
                    message: "Qoder CN chat metadata has invalid fields; no content retained"
                        .into(),
                });
            }
        }
    }
    if out.records_seen == limit {
        out.status = ScanStatus::BudgetExhausted;
    } else {
        cursor.rowid = 0;
    }
    if cursor.invalid {
        out.health = "degraded".into();
    }
    out.parse_context = Some(
        serde_json::json!({"cn_database":true,"validated":!out.events.is_empty(),"fallback":true,"parser_version":PARSER}),
    );
    if cursor.rowid == 0 {
        cursor.invalid = false;
    }
    cursor.parser = PARSER.into();
    out.cursor = Some(serde_json::to_value(cursor)?);
    Ok(out)
}
