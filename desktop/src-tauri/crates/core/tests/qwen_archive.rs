mod common;

use common::{qwen_root_with_file, run_qwen, summary, temp_storage, TempDir};
use llm_usage_core::adapters::framework::{DiscoverContext, SourceAdapter};
use llm_usage_core::adapters::qwen::QwenAdapter;
use std::path::Path;

const NOW: i64 = 1_800_000_000_000;

fn record(id: &str, total: i64) -> String {
    format!(
        r#"{{"uuid":"{id}","sessionId":"session-one","timestamp":"2026-01-05T10:00:00Z","type":"assistant","version":"0.5.0","model":"qwen","usageMetadata":{{"promptTokenCount":{total},"candidatesTokenCount":0,"totalTokenCount":{total}}}}}"#
    )
}

fn count(storage: &llm_usage_core::storage::Storage) -> (i64, Option<i64>) {
    let totals = summary(storage, "2026-01-05", "2026-01-05").totals;
    (totals.call_count, totals.total_tokens_known)
}

#[test]
fn archive_only_then_active_copy_and_tail_do_not_duplicate_calls() {
    let dir = TempDir::new("qwen-archive");
    let root = qwen_root_with_file(
        &dir,
        "project/chats/session-one.jsonl",
        format!("{}\n{}\n", record("one", 10), record("two", 20)).as_bytes(),
    );
    let chats = root.join("tmp/project/chats");
    std::fs::create_dir_all(chats.join("archive")).unwrap();
    std::fs::rename(
        chats.join("session-one.jsonl"),
        chats.join("archive/session-one.jsonl"),
    )
    .unwrap();
    let (_db, storage) = temp_storage("qwen-archive");
    let first = run_qwen(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    assert_eq!(count(&storage), (2, Some(30)));

    // A live writer may recreate the active file after archiving. Its tail is
    // new usage, while the two shared native UUIDs are the same calls.
    std::fs::write(
        chats.join("session-one.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            record("one", 10),
            record("two", 20),
            record("three", 30)
        ),
    )
    .unwrap();
    run_qwen(&storage, &root, NOW + 1);
    assert_eq!(count(&storage), (3, Some(60)));
    let revision = storage.data_revision().unwrap();
    run_qwen(&storage, &root, NOW + 2);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(count(&storage), (3, Some(60)));
}

#[test]
fn old_and_current_layouts_share_identity_and_unrelated_jsonl_is_ignored() {
    let dir = TempDir::new("qwen-projects");
    let root = qwen_root_with_file(
        &dir,
        "old/chats/session-one.jsonl",
        format!("{}\n", record("one", 10)).as_bytes(),
    );
    let current = root.join("projects/current/chats/archive/session-one.jsonl");
    std::fs::create_dir_all(current.parent().unwrap()).unwrap();
    std::fs::write(
        &current,
        format!("{}\n{}\n", record("one", 10), record("two", 20)),
    )
    .unwrap();
    let unrelated = root.join("projects/current/subagents/unrelated.jsonl");
    std::fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
    std::fs::write(unrelated, format!("{}\n", record("extra", 100))).unwrap();
    let discovered = QwenAdapter::new().discover(&DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.clone()],
    });
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].files.len(), 2);
    let (_db, storage) = temp_storage("qwen-projects");
    run_qwen(&storage, &root, NOW);
    assert_eq!(count(&storage), (2, Some(30)));
}

#[test]
fn runtime_override_selects_its_own_tree() {
    let dir = TempDir::new("qwen-runtime-override");
    let home = dir.path().join("home");
    let runtime = dir.path().join("runtime");
    std::fs::create_dir_all(home.join(".qwen/tmp/old/chats")).unwrap();
    std::fs::write(
        home.join(".qwen/tmp/old/chats/old.jsonl"),
        record("old", 10),
    )
    .unwrap();
    std::fs::create_dir_all(runtime.join("projects/new/chats/archive")).unwrap();
    std::fs::write(
        runtime.join("projects/new/chats/archive/new.jsonl"),
        record("new", 20),
    )
    .unwrap();
    let roots = QwenAdapter::new().discover(&DiscoverContext {
        home_dir: Some(home),
        env: [
            (
                "QWEN_RUNTIME_DIR".into(),
                runtime.to_string_lossy().into_owned(),
            ),
            (
                "QWEN_HOME".into(),
                dir.path().join("other").to_string_lossy().into_owned(),
            ),
        ]
        .into(),
        manual_roots: vec![],
    });
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].root, Path::new(&runtime));
    assert_eq!(roots[0].files.len(), 1);
    let unresolved = QwenAdapter::new().discover(&DiscoverContext {
        home_dir: Some(dir.path().join("home")),
        env: [("QWEN_RUNTIME_DIR".into(), "relative-to-agent-cwd".into())].into(),
        manual_roots: vec![],
    });
    assert!(
        unresolved.is_empty(),
        "desktop cwd cannot resolve an agent-relative override"
    );
}
