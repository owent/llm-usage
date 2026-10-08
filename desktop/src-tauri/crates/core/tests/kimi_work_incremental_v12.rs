//! Kimi Work (A13, M4) V12: repeat scans add nothing; append reads and incomplete lines resume;
//! truncation rescans retain imported history, and bounded reads recover in batches.

mod common;

use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::adapters::jsonl::JsonlLimits;
use llm_usage_core::storage::Storage;

const NOW: i64 = 1_800_000_000_000;

fn budgeted_limits(max_lines: u64) -> ScanLimits {
    ScanLimits {
        jsonl: JsonlLimits {
            chunk_bytes: 4096,
            max_line_bytes: 1024 * 1024,
            max_lines: Some(max_lines),
            time_budget: None,
        },
    }
}

fn rec_usage(io: i64, out: i64, cr: i64, time: i64) -> String {
    // Version 1.4 shape: no agentId; model is a bare ID.
    format!(
        r#"{{"type":"usage.record","model":"k3-agent","usage":{{"inputOther":{io},"output":{out},"inputCacheRead":{cr},"inputCacheCreation":0}},"usageScope":"turn","time":{time}}}"#
    )
}

fn base_file() -> Vec<u8> {
    format!(
        "{}\n{}\n{}\n",
        r#"{"type":"metadata","protocol_version":"1.4","created_at":1767225600000}"#,
        rec_usage(100, 50, 400, 1767225601000),
        rec_usage(200, 60, 800, 1767225602000)
    )
    .into_bytes()
}

fn wire_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path()
        .join("sessions")
        .join("wd_syn")
        .join("conv_syn-inc")
        .join("agents")
        .join("main")
        .join("wire.jsonl")
}

fn generation(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("kimi-work-repeat");
    std::fs::create_dir_all(wire_path(&dir).parent().unwrap()).unwrap();
    std::fs::write(wire_path(&dir), base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-work-repeat");

    let first = run_kimi_work(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "complete");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_kimi_work(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "unchanged");
    assert!(second[0].outcome.is_none());
    assert_eq!(storage.data_revision().unwrap(), revision_after_first);

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(1_610));
}

#[test]
fn append_continues_from_cursor() {
    let dir = TempDir::new("kimi-work-append");
    let path = wire_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-work-append");

    let _ = run_kimi_work(&storage, &root, NOW);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(format!("{}\n", rec_usage(300, 70, 900, 1767225603000)).as_bytes());
    std::fs::write(&path, &bytes).unwrap();

    let second = run_kimi_work(&storage, &root, NOW + 1000);
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1);
    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.total_tokens_known, Some(1_610 + 1_270));
}

#[test]
fn half_line_is_not_consumed() {
    let dir = TempDir::new("kimi-work-half");
    let path = wire_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text = format!(
        "{}\n{}",
        r#"{"type":"metadata","protocol_version":"1.4","created_at":1767225600000}"#,
        rec_usage(100, 50, 400, 1767225601000)
    );
    std::fs::write(&path, text.as_bytes()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-work-half");

    let first = run_kimi_work(&storage, &root, NOW);
    assert_eq!(first[0].files[0].events, 0, "半行不消费（仅 metadata）");

    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, &bytes).unwrap();
    let second = run_kimi_work(&storage, &root, NOW + 1000);
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1);
}

#[test]
fn truncation_triggers_rescan_keeps_history() {
    let dir = TempDir::new("kimi-work-trunc");
    let path = wire_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-work-trunc");

    let _ = run_kimi_work(&storage, &root, NOW);
    assert_eq!(generation(&storage), 0);

    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            r#"{"type":"metadata","protocol_version":"1.4","created_at":1767225600000}"#,
            rec_usage(500, 80, 0, 1767225609000)
        )
        .as_bytes(),
    )
    .unwrap();
    let second = run_kimi_work(&storage, &root, NOW + 1000);
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1, "仅新键事件");
    assert_eq!(generation(&storage), 1);

    // Source truncation leaves imported history intact, as in Codex V12.
    let summary = summary(&storage, "2026-01-01", "2026-01-02");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.total_tokens_known, Some(550 + 1_060 + 580));
}

#[test]
fn line_budget_resumes() {
    let dir = TempDir::new("kimi-work-budget");
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/conv_syn-inc/agents/main/wire.jsonl",
        &base_file(),
    );
    let (_db, storage) = temp_storage("kimi-work-budget");

    let first = run_kimi_work_with_limits(&storage, &root, NOW, budgeted_limits(2));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 1);
    assert!(first[0].reconciliations.is_empty(), "未到文件尾不对账");

    let second = run_kimi_work(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1);
    assert!(second[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "echo_subset"));

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
}
