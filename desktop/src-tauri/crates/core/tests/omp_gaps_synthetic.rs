//! oh-my-pi（omp）适配器缺口场景：合成样本（目录/文件头均标 synthetic）与
//! V17/V30 版本策略。覆盖真实样本缺失的场景：四类辅助 usage 载体、无 usage 的
//! assistant、fork 继承去重（已知偏差定案同 pi）、嵌套子 Agent 路径归属、
//! stopReason=error/aborted、cost 映射边界、detect 首行闸口（title/session/Pending）、
//! 未知版本 latest_fallback 兼容回退（V30 新语义，omp 无 evidenced-incompatible
//! 分支，与 pi 不同）、未知格式 fail closed、未知条目类型诊断。
//! 期望值均由 fixture 手工核算（见各 _expectations.md）。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::omp::OmpAdapter;
use llm_usage_core::domain::VersionBasis;
use rusqlite::OptionalExtension;

fn synthetic_root(name: &str) -> std::path::PathBuf {
    omp_fixture(name)
}

fn diag_count(storage: &llm_usage_core::storage::Storage, code: &str) -> i64 {
    storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = ?1",
            [code],
            |r| r.get(0),
        )
        .unwrap()
}

// 手工核算值（synthetic-auxiliary-carriers，9 行 6 事件）：
// primary：syn-a1（100/50/10/5/165）+ syn-a2（无 usage，token 全未知）；
// auxiliary：syn-u1（0/0/0/1000/1000）、syn-c1（200/100/0/0/300）、
// syn-b1（300/150/0/0/450）、syn-t1（10/5/0/0/15）；
// 合计：call_count=6、input_total=1625（派生）、uncached=610、cache_read=10、
// cache_write=1005、output=305、total=1930；usage_shape_deviation ×1（syn-a2）。
#[test]
fn auxiliary_carriers_classified_and_summed() {
    let (_db, storage) = temp_storage("omp-aux");
    let root = synthetic_root("synthetic-auxiliary-carriers");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 9);
    assert_eq!(report.files[0].records_seen, 9);
    assert_eq!(report.files[0].events, 6);
    assert_eq!(report.files[0].diagnostics, 1);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (6, 0, 0));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 6);
    assert_eq!(summary.totals.input_total_known, Some(1_625));
    assert_eq!(summary.totals.uncached_known, Some(610));
    assert_eq!(summary.totals.cache_read_known, Some(10));
    assert_eq!(summary.totals.cache_write_known, Some(1_005));
    assert_eq!(summary.totals.output_total_known, Some(305));
    assert_eq!(summary.totals.total_tokens_known, Some(1_930));
    assert_eq!(summary.totals.input_unknown_count, 1, "syn-a2 无 usage");

    let mut stmt = storage
        .conn()
        .prepare("SELECT call_category, COUNT(*) FROM usage_events GROUP BY call_category ORDER BY call_category")
        .unwrap();
    let categories: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        categories,
        vec![("auxiliary".to_string(), 4), ("primary".to_string(), 2)]
    );

    // 逐键核验归属与分类。
    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT call_category, model_raw, provider_id, model_attribution, input_total
                 FROM usage_events WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .unwrap()
    };
    let a1 = row_for("omp:message:syn-a1:syn-mc1:2026-01-05T10:00:05.000Z");
    assert_eq!(
        a1,
        (
            "primary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "request_field".into(),
            Some(115)
        )
    );
    // 无 usage 的 assistant：计调用，token 全未知（不补零）。
    let a2 = row_for("omp:message:syn-a2:syn-a1:2026-01-05T10:00:06.000Z");
    assert_eq!(a2.0, "primary");
    assert_eq!(a2.4, None, "no usage => tokens unknown");
    let u1 = row_for("omp:usage:syn-u1:syn-a2:2026-01-05T10:00:07.000Z");
    assert_eq!(
        u1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "request_field".into(),
            Some(1000)
        )
    );
    // compaction/branch_summary 无模型字段：按 omp 组合形状 model_change 归属。
    let c1 = row_for("omp:compaction:syn-c1:syn-u1:2026-01-05T10:00:08.000Z");
    assert_eq!(
        c1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "structured_change".into(),
            Some(200)
        )
    );
    let b1 = row_for("omp:branch_summary:syn-b1:syn-c1:2026-01-05T10:00:09.000Z");
    assert_eq!(
        b1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "structured_change".into(),
            Some(300)
        )
    );
    // toolResult 的 usage 是工具执行自身消耗：辅助调用，无模型证据。
    let t1 = row_for("omp:toolresult:syn-t1:syn-b1:2026-01-05T10:00:10.000Z");
    assert_eq!(
        t1,
        ("auxiliary".into(), None, None, "unknown".into(), Some(10))
    );

    // omp 特有浮点毫秒取整：syn-a1 duration 200.4→200、ttft 60.6→61；
    // syn-a2 duration 150.5→151、ttft 缺字段 None。
    let latency = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT duration_ms, ttft_ms FROM usage_events WHERE source_record_key = ?1",
                [key],
                |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?)),
            )
            .unwrap()
    };
    assert_eq!(
        latency("omp:message:syn-a1:syn-mc1:2026-01-05T10:00:05.000Z"),
        (Some(200), Some(61))
    );
    assert_eq!(
        latency("omp:message:syn-a2:syn-a1:2026-01-05T10:00:06.000Z"),
        (Some(151), None)
    );
    assert_eq!(
        latency("omp:usage:syn-u1:syn-a2:2026-01-05T10:00:07.000Z"),
        (None, None),
        "辅助载体无延迟字段"
    );
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
}

// 手工核算值（synthetic-fork-inherited）：源文件 2 事件（150+300），fork 文件逐字
// 复制源 L3–L5 + 新 assistant syn-fa-3（15）；总调用 3，合计 input_total=350
// （派生 input+cacheRead+cacheWrite：(100+0+0)+(200+40+0)+(10+0+0)）、
// cache_read=40、output=115、total=465。已知偏差见 fixture _expectations.md。
#[test]
fn fork_inherited_entries_dedup_across_files() {
    let dir = TempDir::new("omp-fork");
    let case = synthetic_root("synthetic-fork-inherited").join("sessions/--C--syn--");
    let source = std::fs::read(
        case.join("2026-01-05T10-00-00-060Z_00000000-0000-7000-8000-00000000d060.jsonl"),
    )
    .unwrap();
    let fork = std::fs::read(
        case.join("2026-01-05T11-00-00-061Z_00000000-0000-7000-8000-00000000d061.jsonl"),
    )
    .unwrap();
    // 第一轮只放源文件；第二轮补 fork 文件（两文件各扫一次）。
    let root = omp_root_with_file(
        &dir,
        "--C--syn--/2026-01-05T10-00-00-060Z_00000000-0000-7000-8000-00000000d060.jsonl",
        &source,
    );
    let (_db, storage) = temp_storage("omp-fork");
    let first = run_omp(&storage, &root, 1_800_000_000_000);
    assert_eq!(first[0].files.len(), 1);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);

    omp_root_with_file(
        &dir,
        "--C--syn--/2026-01-05T11-00-00-061Z_00000000-0000-7000-8000-00000000d061.jsonl",
        &fork,
    );
    let second = run_omp(&storage, &root, 1_800_000_000_000 + 1000);
    let report = &second[0];
    assert_eq!(report.files.len(), 2);
    assert_eq!(report.files[0].status, "unchanged", "源文件无变化短路");
    assert_eq!(report.files[1].status, "complete");
    assert_eq!(
        report.files[1].events, 3,
        "fork 文件产出 2 复制条目 + 1 新条目"
    );
    let outcome = report.outcome.as_ref().unwrap();
    // 已知偏差（同 pi 定案）：复制条目四元组相同，但事件 session_id/
    // parent_session_id 取自 fork 会话头，与已存事件同键不同内容 → 仲裁 conflict
    // （保留先扫者）。幂等净效果成立：不双计、先扫的源会话归属保留。
    assert_eq!(outcome.added, 1, "仅 fork 自有新条目共入");
    assert_eq!(outcome.conflicts, 2, "复制条目与源会话已存事件冲突");
    assert_eq!(outcome.unchanged, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3, "复制件不双计");
    assert_eq!(summary.totals.input_total_known, Some(350));
    assert_eq!(summary.totals.cache_read_known, Some(40));
    assert_eq!(summary.totals.output_total_known, Some(115));
    assert_eq!(summary.totals.total_tokens_known, Some(465));

    // fork 新条目携带 fork 会话身份与 parentSession。
    let (session, parent): (String, String) = storage
        .conn()
        .query_row(
            "SELECT session_id, parent_session_id FROM usage_events
             WHERE source_record_key = 'omp:message:syn-fa-3:syn-fa-2:2026-01-05T11:00:01.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(session, "syn-omp-fork");
    assert_eq!(parent, "syn-omp-src");

    // 复制条目保留先扫文件（源会话）的归属，并被标记冲突。
    let kept: (String, Option<String>, i64) = storage
        .conn()
        .query_row(
            "SELECT session_id, parent_session_id, conflict FROM usage_events
             WHERE source_record_key = 'omp:message:syn-fa-1:syn-fm-1:2026-01-05T10:00:02.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(kept.0, "syn-omp-src");
    assert_eq!(kept.1, None);
    assert_eq!(kept.2, 1);
    assert_eq!(diag_count(&storage, "update_conflict"), 2);
    let _ = dir;
}

// 手工核算值（synthetic-subagent-nested）：主会话 syn-ma-1（10/5/0/0/15）primary；
// 两层子 Agent Research.jsonl syn-sa-1（100/50/0/0/150）与三层嵌套
// Research/Research.Compactor.jsonl syn-sa-2（200/60/40/0/300）均 sub_agent，
// parent 均为最近的 <ts>_<uuid> 祖先目录 d070。合计 call_count=3、
// input_total=350（派生）、cache_read=40、output=115、total=465、distinct session=3。
#[test]
fn nested_subagent_parent_from_directory_shape() {
    let (_db, storage) = temp_storage("omp-nested");
    let root = synthetic_root("synthetic-subagent-nested");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 3, "主会话 + 两层子 Agent + 三层嵌套");
    for file in &report.files {
        assert_eq!(file.status, "complete");
        assert_eq!(file.events, 1);
    }
    assert_eq!(report.outcome.as_ref().unwrap().added, 3);

    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT call_category, session_id, parent_session_id FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    let main = row_for("omp:message:syn-ma-1:-:2026-01-05T10:00:05.000Z");
    assert_eq!(main.0, "primary");
    assert_eq!(main.1, "syn-omp-main");
    assert_eq!(main.2, None, "主会话无父");
    let parent_uuid = "00000000-0000-7000-8000-00000000d070";
    let sub1 = row_for("omp:message:syn-sa-1:-:2026-01-05T10:01:05.000Z");
    assert_eq!(sub1.0, "sub_agent");
    assert_eq!(sub1.1, "syn-omp-sub1", "子 Agent 自有会话头");
    assert_eq!(sub1.2.as_deref(), Some(parent_uuid));
    let sub2 = row_for("omp:message:syn-sa-2:-:2026-01-05T10:02:05.000Z");
    assert_eq!(sub2.0, "sub_agent");
    assert_eq!(sub2.1, "syn-omp-sub2");
    assert_eq!(
        sub2.2.as_deref(),
        Some(parent_uuid),
        "隔代不归名：归最近的 <ts>_<uuid> 祖先目录"
    );

    let distinct_sessions: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(DISTINCT session_id) FROM usage_events",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(distinct_sessions, 3);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.input_total_known, Some(350));
    assert_eq!(summary.totals.cache_read_known, Some(40));
    assert_eq!(summary.totals.output_total_known, Some(115));
    assert_eq!(summary.totals.total_tokens_known, Some(465));
}

// 手工核算值（synthetic-error-aborted）：2 事件均 primary；syn-e-1 error、
// syn-e-2 aborted（无 duration/ttft）。合计 input_total=25、cache_read=5、
// cache_write=0、output=25、total=50。
#[test]
fn error_and_aborted_stop_reasons_map_to_error_status() {
    let (_db, storage) = temp_storage("omp-err");
    let root = synthetic_root("synthetic-error-aborted");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 2);

    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT error_status, duration_ms, ttft_ms FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    let e1 = row_for("omp:message:syn-e-1:-:2026-01-05T10:00:01.000Z");
    assert_eq!(e1.0, "error");
    assert_eq!(e1.1, Some(100), "duration 100.4→100");
    assert_eq!(e1.2, None, "ttft 缺字段保持 unknown");
    let e2 = row_for("omp:message:syn-e-2:syn-e-1:2026-01-05T10:00:02.000Z");
    assert_eq!(e2.0, "aborted");
    assert_eq!(e2.1, None, "aborted 条目无 duration（真实同形状）");
    assert_eq!(e2.2, None);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(25));
    assert_eq!(summary.totals.cache_read_known, Some(5));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(25));
    assert_eq!(summary.totals.total_tokens_known, Some(50));
}

// 手工核算值（synthetic-cost-estimated）：syn-c1 cost.total=0.058278 → 58278 micro-USD
// estimated；syn-c2 cost.total=0 → 不映射。合计 input_total=105、output=55、total=160。
#[test]
fn cost_total_positive_maps_estimated_zero_stays_unknown() {
    let (_db, storage) = temp_storage("omp-cost");
    let root = synthetic_root("synthetic-cost-estimated");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 2);

    let cost_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT cost_amount_minor, cost_currency, cost_kind FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    assert_eq!(
        cost_for("omp:message:syn-c1:-:2026-01-05T10:00:01.000Z"),
        (Some(58278), Some("USD".into()), Some("estimated".into()))
    );
    assert_eq!(
        cost_for("omp:message:syn-c2:syn-c1:2026-01-05T10:00:02.000Z"),
        (None, None, None),
        "cost.total=0 与无价目不可区分，不映射"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(105));
    assert_eq!(summary.totals.total_tokens_known, Some(160));
}

// 手工核算值（synthetic-detect-supported）：首行 title（v=1）、次行 session v3；
// 5 行 1 事件（syn-a1：100/10/0/0/110，duration 100.4→100、ttft 50.4→50）。
#[test]
fn detect_supported_title_first_and_full_scan() {
    let adapter = OmpAdapter::new();
    let case = synthetic_root("synthetic-detect-supported");
    let path = case.join(
        "sessions/--C--syn--/2026-01-05T10-00-00-010Z_00000000-0000-7000-8000-00000000d010.jsonl",
    );
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: Some("3".to_string()),
            basis: VersionBasis::KnownVersion,
        }
    );

    let (_db, storage) = temp_storage("omp-detect");
    let reports = run_omp(&storage, &case, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 5);
    assert_eq!(report.files[0].records_seen, 5);
    assert_eq!(report.files[0].events, 1);
    assert_eq!(report.outcome.as_ref().unwrap().added, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(
        summary.totals.input_total_known,
        Some(100),
        "派生口径 input+cacheRead+cacheWrite=100+0+0"
    );
    assert_eq!(summary.totals.cache_read_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(10));
    assert_eq!(summary.totals.total_tokens_known, Some(110));

    let latency: (i64, i64) = storage
        .conn()
        .query_row(
            "SELECT duration_ms, ttft_ms FROM usage_events
             WHERE source_record_key = 'omp:message:syn-a1:syn-u1:2026-01-05T10:00:05.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(latency, (100, 50));
}

#[test]
fn detect_session_header_first_also_supported() {
    // omp detect 接受 session 头在前的文件（与 pi 同形；limitations 已记）。
    let dir = TempDir::new("omp-detect-pi-shape");
    let path = dir.path().join("pi-shape.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"session\",\"version\":3,\"id\":\"syn-omp-sh\",\"timestamp\":\"2026-01-05T10:00:00.000Z\"}\n",
    )
    .unwrap();
    let adapter = OmpAdapter::new();
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: Some("3".to_string()),
            basis: VersionBasis::KnownVersion,
        }
    );
    let _ = dir;
}

#[test]
fn detect_pending_when_only_title_without_session() {
    // 只有 title 而前 4 行内无 session 头：可能仍在首次写入中，下轮重探。
    let dir = TempDir::new("omp-detect-title");
    let path = dir.path().join("title-only.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"title\",\"v\":1,\"title\":\"syn-only\",\"updatedAt\":\"2026-01-05T10:00:00.000Z\"}\n",
    )
    .unwrap();
    let adapter = OmpAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
    let _ = dir;
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("omp-empty");
    let path = dir.path().join("empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = OmpAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
    let _ = dir;
}

// 手工核算值（synthetic-unsupported-version，目录名为历史样本组织，V30 起行为
// 已变）：未收录数值版本（version=4）与缺失 version（legacy 形状）都按
// LatestFallback 回退 session_v3 尝试（omp 旧版落盘格式未取证，无证据不兼容，
// 不直接拒绝——与 pi 的 evidenced-incompatible 分支不同）。两文件各 1 事件
// （100/10/0/0/110）：合计 call_count=2、input_total=200（派生）、output=20、
// total=220；事件 parse_basis=latest_fallback；文件 active_compat；
// latest_fallback 诊断每文件一条（框架层探测时记）。
#[test]
fn v17_unknown_version_falls_back_with_compat_mark() {
    let adapter = OmpAdapter::new();
    let case = synthetic_root("synthetic-unsupported-version");
    let v4 = case.join(
        "sessions/--C--syn--/2026-01-05T10-00-00-020Z_00000000-0000-7000-8000-00000000d020.jsonl",
    );
    let legacy = case.join(
        "sessions/--C--syn--/2026-01-05T10-00-00-021Z_00000000-0000-7000-8000-00000000d021.jsonl",
    );
    // 未收录数值版本与缺失 version 都回退最新内置解析器并带兼容标记。
    assert_eq!(
        adapter.detect(&v4).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: Some("4".to_string()),
            basis: VersionBasis::LatestFallback,
        }
    );
    assert_eq!(
        adapter.detect(&legacy).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: None,
            basis: VersionBasis::LatestFallback,
        }
    );

    let (_db, storage) = temp_storage("omp-uv");
    let reports = run_omp(&storage, &case, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 2);
    for file in &report.files {
        assert_eq!(file.status, "complete");
        assert_eq!(file.events, 1, "兼容尝试成功，事件正常产出");
    }
    assert_eq!(
        report.files[0].detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: 4)")
    );
    assert_eq!(
        report.files[1].detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: missing)")
    );
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.conflicts, outcome.errors),
        (2, 0, 0)
    );

    // 兼容尝试成功的数据正常统计。
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(200));
    assert_eq!(summary.totals.output_total_known, Some(20));
    assert_eq!(summary.totals.total_tokens_known, Some(220));

    // 兼容标记持久化：事件 parse_basis、文件 active_compat、探测结论 JSON。
    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT parse_basis, schema_version, session_id FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    assert_eq!(
        row_for("omp:message:syn-m1:-:2026-01-05T10:00:05.000Z"),
        (
            Some("latest_fallback".into()),
            "4".into(),
            Some("syn-omp-v4".into())
        )
    );
    // 缺失 version 的 legacy 形状：schema_version 无从得知记 unknown，basis 仍带标记。
    assert_eq!(
        row_for("omp:message:syn-l1:-:2026-01-05T10:00:05.000Z"),
        (
            Some("latest_fallback".into()),
            "unknown".into(),
            Some("syn-omp-legacy".into())
        )
    );
    assert_eq!(
        diag_count(&storage, "latest_fallback"),
        2,
        "兼容尝试逐文件可见，不是静默处理"
    );
    let statuses: Vec<(String, String)> = {
        let mut stmt = storage
            .conn()
            .prepare("SELECT status, format_status FROM source_files ORDER BY file_id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(statuses.len(), 2);
    for (status, format_status) in &statuses {
        assert_eq!(status, "active_compat");
        let fs: serde_json::Value = serde_json::from_str(format_status).unwrap();
        assert_eq!(fs["basis"], "latest_fallback");
        assert_eq!(fs["compat"], "unverified");
    }
    // file_id 排序：d020（version=4）在前，d021（缺失 version）在后。
    let fs0: serde_json::Value = serde_json::from_str(&statuses[0].1).unwrap();
    let fs1: serde_json::Value = serde_json::from_str(&statuses[1].1).unwrap();
    assert_eq!(fs0["found_version"], "4");
    assert_eq!(fs1["found_version"], serde_json::Value::Null);

    // 重复扫描不增量（兼容标记不改变幂等）。
    let second = run_omp(&storage, &case, 1_800_000_000_000 + 1000);
    let added2: i64 = second
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0);
}

#[test]
fn v17_unknown_format_fails_closed_not_success_zero() {
    let adapter = OmpAdapter::new();
    let path = synthetic_root("synthetic-not-omp").join(
        "sessions/--C--syn--/2026-01-05T10-00-00-030Z_00000000-0000-7000-8000-00000000d030.jsonl",
    );
    let outcome = adapter.detect(&path).unwrap();
    assert!(
        matches!(outcome, DetectOutcome::UnknownFormat { .. }),
        "首行非 title/session ⇒ UnknownFormat（即使第 2 行是合法 session 头）"
    );

    let (_db, storage) = temp_storage("omp-nf");
    let root = synthetic_root("synthetic-not-omp");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "unknown_format");
    assert_eq!(report.files[0].events, 0);
    assert_eq!(diag_count(&storage, "unknown_format"), 1);
}

// 手工核算值（synthetic-unknown-record-type）：1 事件（syn-m1：7/3/0/0/10）；
// 未知类型 brand_new_thing 出现 2 次，诊断每文件每类型只记一次。
#[test]
fn unknown_record_type_ignored_with_single_diagnostic() {
    let (_db, storage) = temp_storage("omp-urt");
    let root = synthetic_root("synthetic-unknown-record-type");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 5);
    assert_eq!(report.files[0].events, 1);
    assert_eq!(report.files[0].diagnostics, 1);
    assert_eq!(diag_count(&storage, "unknown_record_type"), 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(7));
    assert_eq!(summary.totals.output_total_known, Some(3));
    assert_eq!(summary.totals.total_tokens_known, Some(10));
    // 未知类型不 fail closed：文件状态保持 active。
    let status: Option<String> = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .optional()
        .unwrap();
    assert_eq!(status.as_deref(), Some("active"));
}
