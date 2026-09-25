//! Gemini CLI 增量语义（整写 JSON，不是行游标）：重复扫描不增量、追加消息后整文件
//! 重写（upsert 幂等不双计）、半程写入 pending 不推进游标、同长替换触发 generation
//! 重扫、改名身份保持。

mod common;

use common::*;
use llm_usage_core::storage::Storage;

const NOW: i64 = 1_800_000_000_000;

fn msg_user(id: &str, ts: &str) -> String {
    format!(r#"{{"id":"{id}","type":"user","timestamp":"{ts}","content":"synthetic user prompt"}}"#)
}

fn msg_gemini(id: &str, ts: &str, input: i64, output: i64, total: i64) -> String {
    format!(
        r#"{{"id":"{id}","type":"gemini","model":"gemini-3.0-flash","timestamp":"{ts}","content":"synthetic reply {id}","tokens":{{"input":{input},"output":{output},"total":{total}}}}}"#
    )
}

fn session_json(messages: &[String]) -> String {
    format!(
        "{{\"sessionId\":\"syn-sess-inc\",\"projectHash\":\"hash-syn1\",\"startTime\":\"2026-01-05T10:00:00.000Z\",\"lastUpdated\":\"2026-01-05T10:00:30.000Z\",\"messages\":[{}]}}",
        messages.join(",")
    )
}

/// 基础会话：1 user + 2 gemini（input 1000/2000，output 50/100，total 1050/2100）。
/// 手工核算：call_count=2；input_total_known=3000；output_total_known=150；
/// total_tokens_known=3150；cache_read_known=None（未直报，未知不补零）。
fn base_session() -> String {
    session_json(&[
        msg_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        msg_gemini("syn-m-1", "2026-01-05T10:00:05.000Z", 1000, 50, 1050),
        msg_gemini("syn-m-2", "2026-01-05T10:00:12.000Z", 2000, 100, 2100),
    ])
}

fn event_count(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap()
}

fn generation(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("gemini-repeat");
    let root = gemini_root_with_file(
        &dir,
        "hash-1/chats/session-r.json",
        base_session().as_bytes(),
    );
    let (_db, storage) = temp_storage("gemini-repeat");

    let first = run_gemini(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "complete");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_gemini(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].status, "unchanged",
        "已消费字节数 == 文件长度 ⇒ 短路"
    );
    assert!(
        second[0].outcome.is_none(),
        "no commit when nothing changed"
    );
    assert_eq!(storage.data_revision().unwrap(), revision_after_first);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
}

#[test]
fn appended_message_full_rewrite_upserts_idempotently() {
    let dir = TempDir::new("gemini-append");
    let file_path = dir.path().join("tmp/hash-1/chats/session-a.json");
    let root = gemini_root_with_file(
        &dir,
        "hash-1/chats/session-a.json",
        base_session().as_bytes(),
    );
    let (_db, storage) = temp_storage("gemini-append");

    let first = run_gemini(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);

    // 整文件重写：messages 追加一条 gemini（input 500, output 25, total 525）。
    let grown = session_json(&[
        msg_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        msg_gemini("syn-m-1", "2026-01-05T10:00:05.000Z", 1000, 50, 1050),
        msg_gemini("syn-m-2", "2026-01-05T10:00:12.000Z", 2000, 100, 2100),
        msg_gemini("syn-m-3", "2026-01-05T10:00:20.000Z", 500, 25, 525),
    ]);
    std::fs::write(&file_path, grown.as_bytes()).unwrap();
    let second = run_gemini(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(
        second[0].files[0].records_seen, 4,
        "整写 JSON 每次变化全量重读"
    );
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 1, "只有新消息入账");
    assert_eq!(outcome.unchanged, 2, "已入库事件 upsert 幂等");
    assert_eq!(
        generation(&storage),
        0,
        "纯追加不改代（长度单调、创建时间一致）"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3, "重扫不双计");
    assert_eq!(summary.totals.input_total_known, Some(3_500));
    assert_eq!(summary.totals.total_tokens_known, Some(3_675));
}

#[test]
fn half_written_json_pends_then_recovers() {
    let dir = TempDir::new("gemini-half");
    let full = base_session();
    // 截断在 messages 数组中段：保留 sessionId/messages 指纹（detect 仍 Supported），
    // 但 JSON 不完整（半程写入）。
    let cut = full.find("\"content\":\"synthetic reply syn-m-1").unwrap();
    let partial = &full[..cut];
    assert!(partial.contains("\"sessionId\"") && partial.contains("\"messages\""));
    let file_path = dir.path().join("tmp/hash-1/chats/session-h.json");
    let root = gemini_root_with_file(&dir, "hash-1/chats/session-h.json", partial.as_bytes());
    let (_db, storage) = temp_storage("gemini-half");

    let first = run_gemini(&storage, &root, NOW);
    assert_eq!(
        first[0].files[0].status, "pending",
        "parse 失败 ⇒ 暂态 pending"
    );
    assert_eq!(first[0].files[0].events, 0);
    assert_eq!(event_count(&storage), 0);
    let checkpoints: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(checkpoints, 0, "半程写入不推进游标");
    let unparseable: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'session_json_unparseable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unparseable, 1);

    // 补全后下轮正常解析。
    std::fs::write(&file_path, full.as_bytes()).unwrap();
    let second = run_gemini(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 2);

    let third = run_gemini(&storage, &root, NOW + 2000);
    assert_eq!(third[0].files[0].status, "unchanged");
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
}

#[test]
fn same_size_message_swap_triggers_generation_rescan() {
    let dir = TempDir::new("gemini-swap");
    // 两条等长 gemini 消息（同位数数值），交换顺序保持总长不变。
    let msg_a = msg_gemini("syn-m-a", "2026-01-05T10:00:05.000Z", 1000, 50, 1050);
    let msg_b = msg_gemini("syn-m-b", "2026-01-05T10:00:06.000Z", 2000, 60, 2060);
    assert_eq!(msg_a.len(), msg_b.len(), "交换样本必须等长");
    let original = session_json(&[
        msg_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        msg_a.clone(),
        msg_b.clone(),
    ]);
    let swapped = session_json(&[
        msg_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        msg_b,
        msg_a,
    ]);
    assert_eq!(original.len(), swapped.len());
    assert_ne!(original, swapped);
    let file_path = dir.path().join("tmp/hash-1/chats/session-s.json");
    let root = gemini_root_with_file(&dir, "hash-1/chats/session-s.json", original.as_bytes());
    let (_db, storage) = temp_storage("gemini-swap");
    run_gemini(&storage, &root, NOW);
    assert_eq!(generation(&storage), 0);

    std::fs::write(&file_path, swapped.as_bytes()).unwrap();
    let second = run_gemini(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(
        generation(&storage),
        1,
        "同长替换（首指纹变化）⇒ generation+1"
    );
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 0);
    assert_eq!(outcome.unchanged, 2, "事件按稳定身份 upsert 幂等");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2, "重扫不双计");
    assert_eq!(summary.totals.input_total_known, Some(3_000));
    assert_eq!(summary.totals.total_tokens_known, Some(3_110));
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("gemini-rename");
    let old_path = dir.path().join("tmp/hash-1/chats/session-old.json");
    let new_path = dir.path().join("tmp/hash-1/chats/session-new.json");
    let root = gemini_root_with_file(
        &dir,
        "hash-1/chats/session-old.json",
        base_session().as_bytes(),
    );
    let (_db, storage) = temp_storage("gemini-rename");
    run_gemini(&storage, &root, NOW);

    std::fs::rename(&old_path, &new_path).unwrap();
    let second = run_gemini(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files.len(), 1, "同一内容流，不算新文件");
    assert_eq!(second[0].files[0].status, "unchanged");
    assert!(second[0].outcome.is_none(), "改名本身不重扫");
    let file_id: String = storage
        .conn()
        .query_row("SELECT file_id FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert!(file_id.ends_with("session-new.json"));
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
}
