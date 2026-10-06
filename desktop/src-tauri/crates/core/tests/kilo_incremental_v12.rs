//! V12：kilo 适配器增量与刷新语义（SQLite 源，schema 指纹 + message.id
//! 稳定键 + time_updated 已处理位置；对照 codex/pi 的 JSONL 版本）：
//! - 重复扫描不增量（已处理时间窗口内重复行按同键同修订幂等）；
//! - 新增消息只增量入账（不重放全库）；
//! - 行更新（time_updated 提升、tokens 变化）按 source_revision 替换，不冲突；
//! - 未完成消息完成（partial → final）同键替换；
//! - schema 指纹变化 ⇒ 已处理位置重置全量重读（幂等）；
//! - 库文件重建（新文件身份）⇒ 全量重扫幂等，历史不丢。
//!
//! 场景数据为合成（schema 同真实 fixture DDL），数值人工核算。

mod common;

use common::*;

const NOW: i64 = 1_800_000_000_000;
/// 合成时间基；增量各步分别 +1_000_000（远超 60s 重叠窗，避免跨步重读歧义）。
const T0: i64 = 1_783_000_000_000;
const STEP: i64 = 1_000_000;

fn syn_session(version: &str, snapshot: [i64; 5]) -> serde_json::Value {
    serde_json::json!({
        "id": "syn-sess-1",
        "project_id": "syn-proj",
        "parent_id": null,
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

/// 直接写一条 assistant 消息（tokens 可为 Null 表示无 usage；finish 可选）。
#[allow(clippy::too_many_arguments)]
fn insert_message(
    dir: &TempDir,
    id: &str,
    updated: i64,
    tokens: &serde_json::Value,
    finish: Option<&str>,
) {
    let conn = rusqlite::Connection::open(
        dir.path()
            .join(".local")
            .join("share")
            .join("kilo")
            .join("kilo.db"),
    )
    .unwrap();
    let mut data = serde_json::json!({
        "role": "assistant",
        "agent": "code",
        "mode": "code",
        "modelID": "glm-5.2",
        "providerID": "zhipuai-coding-plan",
        "time": {"created": updated, "completed": updated + 500},
        "tokens": tokens,
    });
    if let Some(finish) = finish {
        data["finish"] = serde_json::json!(finish);
    }
    conn.execute(
        "INSERT INTO message (id, session_id, time_created, time_updated, data) \
         VALUES (?1, 'syn-sess-1', ?2, ?2, ?3)",
        rusqlite::params![id, updated, serde_json::to_string(&data).unwrap()],
    )
    .unwrap();
}

fn update_message(
    dir: &TempDir,
    id: &str,
    updated: i64,
    tokens: &serde_json::Value,
    finish: Option<&str>,
) {
    let conn = rusqlite::Connection::open(
        dir.path()
            .join(".local")
            .join("share")
            .join("kilo")
            .join("kilo.db"),
    )
    .unwrap();
    let mut data = serde_json::json!({
        "role": "assistant",
        "agent": "code",
        "mode": "code",
        "modelID": "glm-5.2",
        "providerID": "zhipuai-coding-plan",
        "time": {"created": updated, "completed": updated + 500},
        "tokens": tokens,
    });
    if let Some(finish) = finish {
        data["finish"] = serde_json::json!(finish);
    }
    conn.execute(
        "UPDATE message SET time_updated = ?2, data = ?3 WHERE id = ?1",
        rusqlite::params![id, updated, serde_json::to_string(&data).unwrap()],
    )
    .unwrap();
}

fn base_tokens(input: i64, output: i64) -> serde_json::Value {
    serde_json::json!({
        "input": input, "output": output, "reasoning": 0,
        "cache": {"read": 0, "write": 0}, "total": input + output,
    })
}

#[test]
fn repeat_scan_does_not_double_count() {
    let dir = TempDir::new("kilo-v12-repeat");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([syn_session("7.4.9", [200, 100, 0, 0, 0])]),
            serde_json::json!([
                {
                    "id": "syn-m1", "session_id": "syn-sess-1",
                    "time_created": T0, "time_updated": T0,
                    "data": {
                        "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                        "time": {"created": T0, "completed": T0 + 500},
                        "tokens": {"input": 200, "output": 80, "reasoning": 0,
                                   "cache": {"read": 0, "write": 0}, "total": 280},
                    },
                },
                {
                    "id": "syn-m2", "session_id": "syn-sess-1",
                    "time_created": T0 + 1_000, "time_updated": T0 + 1_000,
                    "data": {
                        "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                        "time": {"created": T0 + 1_000, "completed": T0 + 1_500},
                        "tokens": {"input": 20, "output": 0, "reasoning": 0,
                                   "cache": {"read": 0, "write": 0}, "total": 20},
                    },
                },
            ]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-v12-repeat");

    let first = run_kilo(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    let revision_after_first = storage.data_revision().unwrap();

    // 无变化：已处理时间窗口（60s 重叠）内两行重读，但同键同内容同修订 ⇒ unchanged，
    // 不新增、不双计（SQLite 源不做字节长度短路，见适配器 incremental 约定）。
    let second = run_kilo(&storage, &root, NOW + 1_000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.conflicts),
        (0, 0, 0)
    );
    assert_eq!(outcome.unchanged, 2);

    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(300));
    // 空批次不推进受影响日：修订不再因重算而上抬（仅事务号 +1）。
    assert!(storage.data_revision().unwrap() >= revision_after_first);
    let _ = dir;
}

#[test]
fn appended_message_is_ingested_incrementally() {
    let dir = TempDir::new("kilo-v12-append");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([syn_session("7.4.9", [200, 100, 0, 0, 0])]),
            serde_json::json!([{
                "id": "syn-m1", "session_id": "syn-sess-1",
                "time_created": T0, "time_updated": T0,
                "data": {
                    "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                    "time": {"created": T0, "completed": T0 + 500},
                    "tokens": {"input": 200, "output": 80, "reasoning": 0,
                               "cache": {"read": 0, "write": 0}, "total": 280},
                },
            }]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-v12-append");
    let first = run_kilo(&storage, &root, NOW);
    assert_eq!(first[0].files[0].events, 1);

    // 追加一条（超出重叠窗）：只读已处理位置之后的新行。
    insert_message(
        &dir,
        "syn-m2",
        T0 + STEP,
        &base_tokens(50, 25),
        Some("stop"),
    );
    let second = run_kilo(&storage, &root, NOW + 1_000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 1, "只有新行入账");
    assert_eq!(outcome.unchanged, 1, "重叠窗内旧行幂等");
    // 快照列未同步更新是源侧行为：对账 mismatch 可见（不隐藏），总数以明细为准。
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(280 + 75));
    let _ = dir;
}

#[test]
fn updated_message_replaces_values_by_revision() {
    let dir = TempDir::new("kilo-v12-update");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([syn_session("7.4.9", [200, 80, 0, 0, 0])]),
            serde_json::json!([{
                "id": "syn-m1", "session_id": "syn-sess-1",
                "time_created": T0, "time_updated": T0,
                "data": {
                    "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                    "time": {"created": T0, "completed": T0 + 500},
                    "tokens": {"input": 200, "output": 80, "reasoning": 0,
                               "cache": {"read": 0, "write": 0}, "total": 280},
                },
            }]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-v12-update");
    run_kilo(&storage, &root, NOW);

    // 活库更新旧行：time_updated 提升为修订号，tokens 下修。
    update_message(
        &dir,
        "syn-m1",
        T0 + STEP,
        &base_tokens(150, 60),
        Some("stop"),
    );
    let second = run_kilo(&storage, &root, NOW + 1_000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.conflicts),
        (0, 1, 0)
    );

    let (input, revision): (Option<i64>, Option<i64>) = storage
        .conn()
        .query_row(
            "SELECT input_total, source_revision FROM usage_events \
             WHERE source_record_key = 'kilo:msg:syn-m1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(input, Some(150), "新值替换旧值");
    assert_eq!(revision, Some(T0 + STEP), "source_revision = time_updated");
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(210));
    let _ = dir;
}

#[test]
fn unfinished_message_completion_replaces_partial() {
    let dir = TempDir::new("kilo-v12-partial");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([syn_session("7.4.9", [0; 5])]),
            serde_json::json!([{
                "id": "syn-m1", "session_id": "syn-sess-1",
                "time_created": T0, "time_updated": T0,
                "data": {
                    "role": "assistant", "modelID": "glm-5.2",
                    "time": {"created": T0},
                    "tokens": {"input": 10, "output": 5, "reasoning": 0,
                               "cache": {"read": 0, "write": 0}},
                },
            }]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-v12-partial");
    run_kilo(&storage, &root, NOW);

    let (lifecycle, total, time_basis): (String, Option<i64>, String) = storage
        .conn()
        .query_row(
            "SELECT lifecycle, total_tokens, time_basis FROM usage_events \
             WHERE source_record_key = 'kilo:msg:syn-m1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(lifecycle, "partial");
    assert_eq!(total, Some(15), "未完成消息的中间值照常入账");
    assert_eq!(time_basis, "source_start");

    // 完成后：finish=stop、补 total、time_updated 提升 ⇒ 同键替换为 final。
    update_message(
        &dir,
        "syn-m1",
        T0 + STEP,
        &serde_json::json!({
            "input": 10, "output": 5, "reasoning": 0,
            "cache": {"read": 0, "write": 0}, "total": 15,
        }),
        Some("stop"),
    );
    let second = run_kilo(&storage, &root, NOW + 1_000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.updated, outcome.conflicts),
        (1, 0),
        "final 替换 partial"
    );
    let (lifecycle, time_basis): (String, String) = storage
        .conn()
        .query_row(
            "SELECT lifecycle, time_basis FROM usage_events \
             WHERE source_record_key = 'kilo:msg:syn-m1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(lifecycle, "final");
    assert_eq!(time_basis, "source_completion");
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 1, "同键替换不双计");
    assert_eq!(summary.totals.total_tokens_known, Some(15));
    let _ = dir;
}

#[test]
fn schema_fingerprint_change_resets_watermark_idempotently() {
    let dir = TempDir::new("kilo-v12-schema");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([syn_session("7.4.9", [200, 80, 0, 0, 0])]),
            serde_json::json!([{
                "id": "syn-m1", "session_id": "syn-sess-1",
                "time_created": T0, "time_updated": T0,
                "data": {
                    "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                    "time": {"created": T0, "completed": T0 + 500},
                    "tokens": {"input": 200, "output": 80, "reasoning": 0,
                               "cache": {"read": 0, "write": 0}, "total": 280},
                },
            }]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-v12-schema");
    run_kilo(&storage, &root, NOW);

    // 模拟 schema 演进：篡改持久化的 schema 指纹 ⇒ 下轮已处理位置重置全量重读。
    storage
        .conn()
        .execute(
            "UPDATE ingestion_checkpoints SET parse_context = ?1",
            rusqlite::params![serde_json::json!({
                "schema_fingerprint": "message(evolved)|session(evolved)",
                "version_basis": "known_version",
                "db_version": "7.4.9",
            })
            .to_string()],
        )
        .unwrap();
    let second = run_kilo(&storage, &root, NOW + 1_000);
    // 全量重读：旧行同键同修订同内容 ⇒ unchanged，不重加、不冲突。
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.unchanged),
        (0, 0, 1)
    );
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(280));
    let _ = dir;
}

#[test]
fn recreated_database_rescans_idempotently() {
    let dir = TempDir::new("kilo-v12-recreate");
    let db_path = dir
        .path()
        .join(".local")
        .join("share")
        .join("kilo")
        .join("kilo.db");
    let root = build_kilo_db(
        &dir,
        &synthetic_kilo_projection(
            serde_json::json!([syn_session("7.4.9", [200, 80, 0, 0, 0])]),
            serde_json::json!([{
                "id": "syn-m1", "session_id": "syn-sess-1",
                "time_created": T0, "time_updated": T0,
                "data": {
                    "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                    "time": {"created": T0, "completed": T0 + 500},
                    "tokens": {"input": 200, "output": 80, "reasoning": 0,
                               "cache": {"read": 0, "write": 0}, "total": 280},
                },
            }]),
        ),
    );
    let (_db, storage) = temp_storage("kilo-v12-recreate");
    run_kilo(&storage, &root, NOW);

    // 同路径重建（新文件身份）：全量重扫，同实例同键幂等，历史不丢。
    let projection = synthetic_kilo_projection(
        serde_json::json!([syn_session("7.4.9", [200, 80, 0, 0, 0])]),
        serde_json::json!([{
            "id": "syn-m1", "session_id": "syn-sess-1",
            "time_created": T0, "time_updated": T0,
            "data": {
                "role": "assistant", "finish": "stop", "modelID": "glm-5.2",
                "time": {"created": T0, "completed": T0 + 500},
                "tokens": {"input": 200, "output": 80, "reasoning": 0,
                           "cache": {"read": 0, "write": 0}, "total": 280},
            },
        }]),
    );
    std::fs::remove_file(&db_path).unwrap();
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
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
                200,
                80,
                0,
                0,
                0,
                None::<String>,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) \
             VALUES ('syn-m1', 'syn-sess-1', ?1, ?1, ?2)",
            rusqlite::params![
                T0,
                serde_json::to_string(&projection["messages"][0]["data"]).unwrap(),
            ],
        )
        .unwrap();
    }
    let second = run_kilo(&storage, &root, NOW + 1_000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.unchanged),
        (0, 1),
        "重扫同键幂等（新身份全量重读或同身份处理位置重读都收敛于此）"
    );
    // 文件身份 = 创建时间 + 首采样指纹：创建时间不可得（None）且内容同形的
    // 重建库会命中旧身份（框架按身份改名探测语义复用行），不产生新实例数据。
    let identities: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(DISTINCT file_identity) FROM source_files",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(identities, 1, "同路径同内容重建收敛到同一内容流身份");
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 1, "历史不丢也不双计");
    assert_eq!(summary.totals.total_tokens_known, Some(280));
    let _ = dir;
}
