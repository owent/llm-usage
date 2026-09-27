//! zcode db 回填回归（2026-09-27 清空重采数据丢失恢复）：
//! 与已入库 JSONL 事件行对行去重（session + ±2s + token 相等）、重跑幂等、
//! token/分类/error_status 映射、未证 querySource 一次诊断。
mod common;

use common::{evt, temp_storage, with_tokens, TempDir};
use llm_usage_core::adapters::zcode::db_backfill::{zcode_db_backfill, ZCODE_DB_BACKFILL_VERSION};
use llm_usage_core::domain::{FieldQuality, RecordKind};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::storage::Storage;
use rusqlite::Connection;
use std::path::PathBuf;

const T0: i64 = 1_790_400_000_000; // 任意固定基准

/// 建合成 zcode db（model_usage 仅含回填读取的列）。
fn build_zcode_db(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("zcode-db.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE model_usage (
           logical_request_id TEXT, attempt_index INTEGER, session_id TEXT,
           provider_id TEXT, model_id TEXT, query_source TEXT,
           completed_at INTEGER, duration_ms INTEGER,
           input_tokens INTEGER, cache_read_input_tokens INTEGER,
           cache_creation_input_tokens INTEGER, output_tokens INTEGER,
           reasoning_tokens INTEGER, provider_total_tokens INTEGER, status TEXT
         );",
    )
    .unwrap();
    let insert = |conn: &Connection, vals: &[&dyn rusqlite::ToSql]| {
        conn.execute(
            "INSERT INTO model_usage VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            vals,
        )
        .unwrap();
    };
    // (a) 与已入库事件同调用：completed_at 晚 20ms，token 相等 → 匹配剔除。
    insert(
        &conn,
        &[
            &"req-a", &0i64, &"sess-1", &"zhipu", &"GLM-5.3", &"main_turn",
            &(T0 + 20), &Some(100i64),
            &1000i64, &Some(800i64), &Some(0i64), &100i64, &Some(0i64),
            &Some(1100i64), &"completed",
        ],
    );
    // (b) 新调用：main_turn。
    insert(
        &conn,
        &[
            &"req-b", &0i64, &"sess-1", &"zhipu", &"GLM-5.3", &"main_turn",
            &(T0 + 60_000), &Some(123i64),
            &500i64, &Some(400i64), &Some(0i64), &50i64, &Some(0i64),
            &Some(550i64), &"completed",
        ],
    );
    // (c) 新调用：subagent + cancelled（token 已上报，回填且带 error_status）。
    insert(
        &conn,
        &[
            &"req-c", &0i64, &"sess-sub-1", &"zhipu", &"GLM-5.3", &"subagent",
            &(T0 + 120_000), &None::<i64>,
            &200i64, &Some(150i64), &Some(50i64), &20i64, &None::<i64>,
            &Some(220i64), &"cancelled",
        ],
    );
    // (d) 新调用：compact（未证值域 → unknown + 一次诊断）。
    insert(
        &conn,
        &[
            &"req-d", &0i64, &"sess-1", &"zhipu", &"GLM-5.3", &"compact",
            &(T0 + 180_000), &None::<i64>,
            &30i64, &Some(0i64), &Some(0i64), &5i64, &None::<i64>,
            &Some(35i64), &"completed",
        ],
    );
    drop(conn);
    path
}

fn zcode_event(key: &str, session: &str, occurred: i64, input: i64, output: i64) -> llm_usage_core::domain::EventInput {
    let mut e = with_tokens(evt("zcode@test", key, occurred), input, output);
    e.agent = "zcode".to_string();
    e.session_id = Some(session.to_string());
    e
}

#[test]
fn backfill_dedups_against_existing_and_is_idempotent() {
    let (_dir, storage) = temp_storage("zcode-backfill");
    let dir = TempDir::new("zcode-backfill-db");
    let db = build_zcode_db(&dir);

    // 已入库的 JSONL 事件：与 db 行 (a) 同调用（Δt=20ms，token 相等）。
    let existing = zcode_event("zcode:req-x:1", "sess-1", T0, 1000, 100);
    commit_batch(
        &storage,
        &llm_usage_core::ingest::IngestBatch {
            batch_id: "seed".into(),
            instance_id: "zcode@test".into(),
            timezone: "Asia/Shanghai".into(),
            now_ms: T0 + 1,
            events: vec![existing],
            checkpoints: vec![],
            diagnostics: vec![],
            run_id: None,
            retention_cutoff_ms: None,
        },
        None,
    )
    .unwrap();

    // 第一次回填：(a) 匹配剔除，(b)(c)(d) 新增。
    let out = zcode_db_backfill(&storage, &db, "zcode@test", "Asia/Shanghai", T0 + 2).unwrap();
    assert_eq!(out.db_rows, 4);
    assert_eq!(out.matched_existing, 1);
    assert_eq!(out.added, 3);

    // 事件字段核对。
    let conn = storage.conn();
    let (cat, err, unc, cre, key_c): (String, Option<String>, Option<i64>, Option<i64>, String) = conn
        .query_row(
            "SELECT call_category, error_status, input_uncached, input_cache_read, source_record_key
             FROM usage_events WHERE source_record_key = 'zcodedb:req-c:1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!((cat.as_str(), err.as_deref(), unc, cre), ("sub_agent", Some("cancelled"), Some(0), Some(150)));
    assert_eq!(key_c, "zcodedb:req-c:1");
    let (cat_d, pv): (String, String) = conn
        .query_row(
            "SELECT call_category, parser_version FROM usage_events WHERE source_record_key = 'zcodedb:req-d:1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(cat_d, "unknown");
    assert_eq!(pv, ZCODE_DB_BACKFILL_VERSION);
    // 未证 querySource 只发一次诊断。
    let diag: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unmapped_query_source'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);

    // 聚合层：当日 token 含 (b)(c)(d)（(a) 在既有事件里已计入）。
    // Asia/Shanghai 当日 = T0 所在日；直接对 daily_usage 求 input 已知总量。
    let input_sum: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(input_known_sum), 0) FROM daily_usage WHERE agent = 'zcode'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(input_sum, 1000 + 500 + 200 + 30);

    // 幂等：重跑全部匹配（回填事件同样参与行对行匹配），零新增。
    let out2 = zcode_db_backfill(&storage, &db, "zcode@test", "Asia/Shanghai", T0 + 3).unwrap();
    assert_eq!(out2.db_rows, 4);
    assert_eq!(out2.matched_existing, 4);
    assert_eq!(out2.added, 0);

    // 时间窗外的同 token 行不算同一调用（Δt=5s > 2s 容差）。
    let conn2 = Connection::open(&db).unwrap();
    conn2
        .execute(
            "INSERT INTO model_usage VALUES ('req-e',0,'sess-1','zhipu','GLM-5.3','main_turn',?,?,500,400,0,50,0,550,'completed')",
            rusqlite::params![T0 + 300_000 + 5_000 + 60, 90i64],
        )
        .unwrap();
    drop(conn2);
    let out3 = zcode_db_backfill(&storage, &db, "zcode@test", "Asia/Shanghai", T0 + 4).unwrap();
    assert_eq!(out3.db_rows, 5);
    assert_eq!(out3.matched_existing, 4);
    assert_eq!(out3.added, 1);
}

/// record_kind 校验：回填事件为 model_call（匹配查询的过滤条件依赖它）。
#[test]
fn backfill_events_are_model_calls() {
    let (_dir, storage) = temp_storage("zcode-backfill-kind");
    let dir = TempDir::new("zcode-backfill-kind-db");
    let db = build_zcode_db(&dir);
    // 全新库：无已入库事件，4 行全部新增。
    let out = zcode_db_backfill(&storage, &db, "zcode@test", "Asia/Shanghai", T0).unwrap();
    assert_eq!(out.added, 4);
    let kind: String = storage
        .conn()
        .query_row(
            "SELECT record_kind FROM usage_events WHERE source_record_key = 'zcodedb:req-b:1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kind, RecordKind::ModelCall.as_str());
    let quality_input: String = storage
        .conn()
        .query_row(
            "SELECT json_extract(quality_json, '$.input_total') FROM usage_events WHERE source_record_key = 'zcodedb:req-b:1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(quality_input, FieldQuality::Reported.as_str());
}
