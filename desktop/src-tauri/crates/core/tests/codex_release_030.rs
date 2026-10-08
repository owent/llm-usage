//! Native version samples and old-cursor recovery for the 0.3.0 Codex corrections.
mod common;
use common::*;
use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::domain::VersionBasis;

#[test]
fn checked_native_versions_select_their_format_and_preserve_usage_on_repeat() {
    let root = codex_fixture("releases-030");
    let expectations: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("expectations.json")).unwrap())
            .unwrap();
    for exp in expectations.as_array().unwrap() {
        let version = exp["version"].as_str().unwrap();
        let bytes = reconstruct_codex_jsonl(&root.join(exp["fixture"].as_str().unwrap()));
        let dir = TempDir::new("codex-release-030");
        let source = codex_root_with_file(&dir, "rollout-native.jsonl", &bytes);
        let detected = CodexAdapter::new()
            .detect(&source.join("sessions/2026/09/24/rollout-native.jsonl"))
            .unwrap();
        assert!(
            matches!(
                detected,
                DetectOutcome::Supported {
                    basis: VersionBasis::KnownVersion,
                    ..
                }
            ),
            "{version}: {detected:?}"
        );
        let (_db, storage) = temp_storage("codex-release-030");
        let reports = run_codex(&storage, &source, 1_800_000_000_000);
        assert!(
            reports[0].error.is_none(),
            "{version}: {:?}",
            reports[0].error
        );
        let totals = summary(&storage, "2020-01-01", "2100-01-01").totals;
        assert_eq!(
            totals.call_count,
            exp["calls"].as_i64().unwrap(),
            "{version}"
        );
        for (name, actual) in [
            ("input", totals.input_total_known),
            ("cached", totals.cache_read_known),
            ("write", totals.cache_write_known),
            ("output", totals.output_total_known),
            ("total", totals.total_tokens_known),
        ] {
            assert_eq!(actual, exp[name].as_i64(), "{version}: {name}");
        }
        let reasoning: i64 = storage
            .conn()
            .query_row("SELECT SUM(output_reasoning) FROM usage_events", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(reasoning, exp["reasoning"].as_i64().unwrap(), "{version}");
        let status: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "active", "{version}");
        run_codex(&storage, &source, 1_800_000_000_001);
        assert_eq!(
            summary(&storage, "2020-01-01", "2100-01-01").totals,
            totals,
            "{version}"
        );
        let diagnostics: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
            .unwrap();
        storage.conn().execute_batch("UPDATE source_instances SET parser_version='codex-rollout-4'; UPDATE usage_events SET parser_version='codex-rollout-4'; UPDATE source_files SET status='degraded';").unwrap();
        let replay = run_codex(&storage, &source, 1_800_000_000_002);
        assert!(
            replay[0].files[0].lines_read > 0,
            "{version}: consumed unchanged bytes must be checked again"
        );
        assert_eq!(
            summary(&storage, "2020-01-01", "2100-01-01").totals,
            totals,
            "{version}: replay must preserve usage"
        );
        let recovered: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(recovered, "active", "{version}");
        let retained: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
            .unwrap();
        assert!(
            retained >= diagnostics,
            "{version}: retain diagnostic history"
        );
    }
}

#[test]
fn absent_or_null_compaction_usage_is_optional_but_invalid_objects_remain_errors() {
    let path = codex_fixture("releases-030/rollout-0.153.4.sanitized.json");
    let original = reconstruct_codex_jsonl(&path);
    for (replacement, expected) in [
        (None, "active"),
        (Some(serde_json::Value::Null), "active"),
        (
            Some(serde_json::json!({"usage":{"input_tokens":"invalid"}})),
            "degraded",
        ),
    ] {
        let mut rows: Vec<serde_json::Value> = std::str::from_utf8(&original)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let row = rows
            .iter_mut()
            .find(|row| {
                row["type"] == "compacted" && row["payload"]["latest_token_usage_record"].is_null()
            })
            .unwrap();
        match replacement {
            Some(value) => row["payload"]["latest_token_usage_record"] = value,
            None => {
                row["payload"]
                    .as_object_mut()
                    .unwrap()
                    .remove("latest_token_usage_record");
            }
        }
        let text = rows
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let dir = TempDir::new("codex-compaction-null");
        let root = codex_root_with_file(&dir, "rollout-compaction.jsonl", text.as_bytes());
        let (_db, storage) = temp_storage("codex-compaction-null");
        run_codex(&storage, &root, 1_800_000_000_000);
        let status: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, expected);
        assert_eq!(
            summary(&storage, "2020-01-01", "2100-01-01")
                .totals
                .call_count,
            25
        );
    }
}
