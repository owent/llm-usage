//! Kilo missing/invalid-data tests: every kilo.db here is synthetic.
//! Schema DDL comes from real samples; token values are calculated manually.
//!
//! Covers assistant messages without usage, unknown session.version latest_fallback,
//! valid rows alongside bad JSON/orphan messages and reported/derived-total mismatch.
//! Reject session_message-only databases lacking message;
//! retain earlier results with a busy writer under read-only requirement 4.

mod common;

use common::*;

const NOW: i64 = 1_800_000_000_000;
/// Synthetic message baseline: epoch milliseconds, 2026-07-02T13:46:40Z.
const T0: i64 = 1_783_000_000_000;

fn session(id: &str, parent: Option<&str>, version: &str, snapshot: [i64; 5]) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "project_id": "syn-proj",
        "parent_id": parent,
        "slug": "syn",
        "directory": "syn",
        "title": "syn",
        "version": version,
        "share_url": null,
        "summary_additions": 0,
        "summary_deletions": 0,
        "summary_files": 0,
        "summary_diffs": null,
        "revert": null,
        "permission": null,
        "time_created": T0,
        "time_updated": T0,
        "time_compacting": null,
        "time_archived": null,
        "workspace_id": null,
        "path": "syn",
        "agent": "",
        "model": "code",
        "cost": 0.0,
        "tokens_input": snapshot[0],
        "tokens_output": snapshot[1],
        "tokens_reasoning": snapshot[2],
        "tokens_cache_read": snapshot[3],
        "tokens_cache_write": snapshot[4],
        "metadata": null,
    })
}

fn assistant(
    id: &str,
    session_id: &str,
    updated: i64,
    tokens: serde_json::Value,
) -> serde_json::Value {
    message(
        id,
        session_id,
        updated,
        serde_json::json!({
            "role": "assistant",
            "agent": "code",
            "mode": "code",
            "finish": "stop",
            "modelID": "glm-5.2",
            "providerID": "zhipuai-coding-plan",
            "time": {"created": T0, "completed": T0 + 1_000},
            "tokens": tokens,
        }),
    )
}

fn message(id: &str, session_id: &str, updated: i64, data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "session_id": session_id,
        "time_created": T0,
        "time_updated": updated,
        "data": data,
    })
}

fn full_tokens(
    input: i64,
    output: i64,
    reasoning: i64,
    read: i64,
    write: i64,
    total: i64,
) -> serde_json::Value {
    serde_json::json!({
        "input": input, "output": output, "reasoning": reasoning,
        "cache": {"read": read, "write": write}, "total": total,
    })
}

#[test]
fn assistant_without_tokens_counts_call_with_unknown_usage() {
    // syn-a-1: assistant without tokens counts one call with unknown token fields.
    let dir = TempDir::new("kilo-no-usage");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([session("syn-sess-1", None, "7.4.9", [0; 5])]),
            serde_json::json!([message(
                "syn-a-1",
                "syn-sess-1",
                T0,
                serde_json::json!({
                    "role": "assistant", "agent": "code", "modelID": "glm-5.2",
                    "time": {"created": T0},
                }),
            )]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-no-usage");
    let reports = run_kilo(&storage, &root, NOW);

    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(reports[0].files[0].diagnostics, 1, "usage_shape_deviation");
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 1);
    // quality_bucket=unknown counts the call but has no known fields or unknown-field denominators.
    assert_eq!(summary.totals.input_unknown_count, 0);
    assert_eq!(summary.totals.input_total_known, None);
    assert_eq!(summary.totals.total_tokens_known, None);
    let codes: Vec<String> = {
        let mut stmt = storage
            .conn()
            .prepare("SELECT code FROM diagnostics")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(codes, vec!["usage_shape_deviation".to_string()]);
    let _ = dir;
}

#[test]
fn unknown_version_falls_back_to_latest_parser_with_compat_flag() {
    // Unregistered session.version=9.9.9 uses latest_fallback and retains compatibility status.
    let dir = TempDir::new("kilo-fallback");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([session("syn-sess-1", None, "9.9.9", [100, 50, 10, 0, 0])]),
            serde_json::json!([assistant(
                "syn-a-1",
                "syn-sess-1",
                T0,
                full_tokens(100, 50, 10, 0, 0, 160),
            )]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-fallback");
    let reports = run_kilo(&storage, &root, NOW);

    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1);
    assert!(reports[0].files[0]
        .detail
        .as_deref()
        .is_some_and(|d| d.contains("latest_fallback")));
    let conn = storage.conn();
    let basis: String = conn
        .query_row("SELECT parse_basis FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(basis, "latest_fallback");
    let total: Option<i64> = conn
        .query_row("SELECT total_tokens FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, Some(160), "兼容尝试照常映射");
    let fallback_diags: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'latest_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fallback_diags, 1, "框架推送 latest_fallback 诊断");
    let _ = dir;
}

#[test]
fn partial_availability_isolates_bad_rows_and_counts_orphan_messages() {
    // syn-good is valid; syn-bad has invalid data JSON and receives diagnostics.
    // syn-orphan references a missing session; LEFT JOIN still counts the call with no_snapshot.
    let dir = TempDir::new("kilo-partial");
    let mut messages = vec![
        assistant(
            "syn-good",
            "syn-sess-1",
            T0,
            full_tokens(200, 20, 5, 30, 0, 255),
        ),
        message(
            "syn-bad",
            "syn-sess-1",
            T0,
            serde_json::json!({"role": "assistant"}),
        ),
        message(
            "syn-orphan",
            "syn-sess-missing",
            T0,
            serde_json::json!({
                "role": "assistant", "modelID": "glm-5.2",
                "time": {"created": T0, "completed": T0 + 1},
                "tokens": full_tokens(10, 1, 0, 0, 0, 11),
            }),
        ),
    ];
    // Write syn-bad data as invalid JSON text directly, bypassing sanitized-data serialization.
    let root = {
        let projection = synthetic_kilo_projection(
            serde_json::json!([session("syn-sess-1", None, "7.4.9", [230, 21, 5, 30, 0])]),
            serde_json::json!([]),
        );
        let kilo_home = dir.path().join(".local").join("share").join("kilo");
        std::fs::create_dir_all(&kilo_home).unwrap();
        let conn = rusqlite::Connection::open(kilo_home.join("kilo.db")).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        conn.execute_batch(projection["schema"]["message_ddl"].as_str().unwrap())
            .unwrap();
        conn.execute_batch(projection["schema"]["session_ddl"].as_str().unwrap())
            .unwrap();
        conn.execute(
            "INSERT INTO session (id, project_id, parent_id, slug, directory, title, version, \
             share_url, summary_additions, summary_deletions, summary_files, summary_diffs, \
             revert, permission, time_created, time_updated, time_compacting, time_archived, \
             workspace_id, path, agent, model, cost, tokens_input, tokens_output, \
             tokens_reasoning, tokens_cache_read, tokens_cache_write, metadata) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,\
             ?21,?22,?23,?24,?25,?26,?27,?28,?29)",
            rusqlite::params![
                "syn-sess-1",
                "syn-proj",
                None::<String>,
                "syn",
                "syn",
                "syn",
                "7.4.9",
                None::<String>,
                0,
                0,
                0,
                None::<String>,
                None::<String>,
                None::<String>,
                T0,
                T0,
                None::<i64>,
                None::<i64>,
                None::<String>,
                "syn",
                "",
                "code",
                0.0,
                230,
                21,
                5,
                30,
                0,
                None::<String>,
            ],
        )
        .unwrap();
        for m in messages.drain(..) {
            let data_text = if m["id"].as_str() == Some("syn-bad") {
                "{\"role\": \"assistant\", broken".to_string()
            } else {
                serde_json::to_string(&m["data"]).unwrap()
            };
            conn.execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    m["id"].as_str().unwrap(),
                    m["session_id"].as_str().unwrap(),
                    m["time_created"].as_i64().unwrap(),
                    m["time_updated"].as_i64().unwrap(),
                    data_text,
                ],
            )
            .unwrap();
        }
        drop(conn);
        dir.path().to_path_buf()
    };

    let (_db, storage) = temp_storage("kilo-partial");
    let reports = run_kilo(&storage, &root, NOW);

    // Retain two events, good and orphan, while diagnosing the bad row.
    assert_eq!(reports[0].files[0].events, 2);
    let conn = storage.conn();
    let bad: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'bad_data_json'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 1);
    // The orphan counts as a call and has no_snapshot reconciliation.
    let (key, category, schema_version): (String, String, String) = conn
        .query_row(
            "SELECT source_record_key, call_category, schema_version FROM usage_events \
             WHERE source_record_key = 'kilo:msg:syn-orphan'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(key, "kilo:msg:syn-orphan");
    assert_eq!(category, "primary", "无父会话证据按主调用");
    assert_eq!(schema_version, "unknown");
    let verdicts: Vec<(String, String)> = reports[0]
        .reconciliations
        .iter()
        .map(|r| (r.series.clone(), r.verdict.clone()))
        .collect();
    assert!(verdicts.contains(&(
        "session_cumulative_snapshot".to_string(),
        "no_snapshot".to_string()
    )));
    // Unassigned bad JSON leaves 255 as a valid-row subtotal, not a complete total to compare with snapshot 286.
    assert!(reports[0].reconciliations.iter().any(|r| {
        r.verdict == "detail_incomplete"
            && r.detail_sum == 255
            && r.snapshot_final == Some(286)
            && r.difference.is_none()
    }));
    let health: String = conn
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        health, "degraded",
        "invalid JSON remains a real record error"
    );
    let _ = dir;
}

#[test]
fn reported_total_mismatch_is_diagnosed_not_hidden() {
    // Reported total 999 differs from derived 255; diagnose source_total_mismatch without clipping.
    let dir = TempDir::new("kilo-mismatch");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([session("syn-sess-1", None, "7.4.9", [0; 5])]),
            serde_json::json!([assistant(
                "syn-a-1",
                "syn-sess-1",
                T0,
                full_tokens(200, 20, 5, 30, 0, 999),
            )]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-mismatch");
    let reports = run_kilo(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 1);
    let conn = storage.conn();
    let (code, total, source_total): (String, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT d.code, e.total_tokens, e.source_total \
             FROM diagnostics d JOIN usage_events e ON e.source_record_key = d.position \
             WHERE d.code = 'source_total_mismatch'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(code, "source_total_mismatch");
    assert_eq!(total, Some(255), "规范化总量按派生口径");
    assert_eq!(source_total, Some(999), "直报值独立保留");
    let _ = dir;
}

#[test]
fn core_data_layer_without_message_table_fails_closed() {
    // session_message without message is unknown_format and is not read as a supported format.
    let dir = TempDir::new("kilo-core-layer");
    let kilo_home = dir.path().join(".local").join("share").join("kilo");
    std::fs::create_dir_all(&kilo_home).unwrap();
    {
        let conn = rusqlite::Connection::open(kilo_home.join("kilo.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE session_message (id text PRIMARY KEY, session_id text NOT NULL, \
             type text NOT NULL, seq integer, time_created integer NOT NULL, \
             time_updated integer NOT NULL, data text NOT NULL);",
        )
        .unwrap();
    }
    let outcome = kilo_detect(&kilo_home.join("kilo.db"));
    assert!(
        matches!(
            outcome,
            llm_usage_core::adapters::framework::DetectOutcome::UnknownFormat { .. }
        ),
        "session_message-only 库应 fail closed：{outcome:?}"
    );

    let (_db, storage) = temp_storage("kilo-core-layer");
    let reports = run_kilo(&storage, dir.path(), NOW);
    assert_eq!(reports[0].files[0].status, "unknown_format");
    assert_eq!(reports[0].files[0].events, 0);
    let codes: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unknown_format'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(codes, 1);
    let _ = dir;
}

#[test]
fn busy_writer_keeps_old_results() {
    // Read-only requirement 4: an exclusive rollback-journal writer blocks a consistent read.
    // Staging backup also fails to acquire the lock; the source time limit interrupts, retaining results/cursor.
    let dir = TempDir::new("kilo-busy");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([session("syn-sess-1", None, "7.4.9", [230, 21, 5, 30, 0])]),
            serde_json::json!([assistant(
                "syn-a-1",
                "syn-sess-1",
                T0,
                full_tokens(200, 20, 5, 30, 0, 255),
            )]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-busy");
    let first = run_kilo(&storage, &root, NOW);
    assert_eq!(first[0].files[0].events, 1);
    let summary_before = summary(&storage, "2026-01-01", "2026-12-31");
    let cursors_before: Vec<String> = storage
        .conn()
        .prepare("SELECT cursor_value FROM ingestion_checkpoints ORDER BY scope_key")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    // Another connection holds EXCLUSIVE; neither the short read nor staging backup can read consistently.
    let holder = rusqlite::Connection::open(
        dir.path()
            .join(".local")
            .join("share")
            .join("kilo")
            .join("kilo.db"),
    )
    .unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    let second = run_kilo(&storage, &root, NOW + 1_000);
    assert_eq!(
        second[0].finish,
        llm_usage_core::jobs::RunStatus::Interrupted
    );
    assert!(second[0]
        .error
        .as_deref()
        .is_some_and(|d| d.contains("budget_exhausted")));
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT status FROM ingest_runs WHERE run_id=?1",
                [second[0].run_id.as_ref().unwrap()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "interrupted"
    );
    let cursors_after: Vec<String> = storage
        .conn()
        .prepare("SELECT cursor_value FROM ingestion_checkpoints ORDER BY scope_key")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(cursors_before, cursors_after);
    // Retain old totals and history, without replacing them with zero.
    let summary_after = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(
        summary_after.totals.call_count,
        summary_before.totals.call_count
    );
    assert_eq!(
        summary_after.totals.total_tokens_known,
        summary_before.totals.total_tokens_known
    );
    holder.execute_batch("ROLLBACK;").unwrap();
    drop(holder);
    // Collection resumes after the lock is released.
    let third = run_kilo(&storage, &root, NOW + 2_000);
    assert_eq!(third[0].files[0].status, "complete");
    // Remove staging copies after use; this process leaves none in the system temporary directory.
    let prefix = format!("llm-usage-kilo-staging-{}-", std::process::id());
    let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with(&prefix))
                })
                .collect()
        })
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "staging leftovers: {leftovers:?}");
    let _ = dir;
}
