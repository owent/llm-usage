//! Route app-owned carriers explicitly and preserve physical-file ownership.
use super::framework::{normalize_path, DetectOutcome, DiscoverContext, SourceAdapter};
use crate::{error::CoreError, storage::Storage};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

pub fn context_for_adapter(
    base: &DiscoverContext,
    adapter_id: &str,
    copilot_otel_roots: &[PathBuf],
) -> DiscoverContext {
    let mut ctx = base.clone();
    if adapter_id != "qwen" {
        ctx.manual_roots.retain(|root| !qwen_native_root(root));
    }
    if adapter_id != "zed" {
        ctx.manual_roots.retain(|root| !zed_native_file(root));
    }
    if adapter_id == "otel" {
        ctx.manual_roots.extend_from_slice(copilot_otel_roots);
    }
    ctx
}

/// Keep valid history and settings. Only hide empty instances created by the old
/// managed-root broadcast; a future real discovery makes them active again.
pub fn retire_misrouted_sources(storage: &Storage, roots: &[PathBuf]) -> Result<(), CoreError> {
    retire_other_sources(storage, roots, "otel")
}

pub fn retire_misrouted_qwen_sources(
    storage: &Storage,
    ctx: &DiscoverContext,
) -> Result<(), CoreError> {
    let roots: Vec<_> = ctx
        .manual_roots
        .iter()
        .filter(|root| qwen_native_root(root))
        .cloned()
        .collect();
    retire_other_sources(storage, &roots, "qwen")
}

pub fn retire_misrouted_zed_sources(
    storage: &Storage,
    ctx: &DiscoverContext,
) -> Result<(), CoreError> {
    let roots: Vec<_> = ctx
        .manual_roots
        .iter()
        .filter(|root| zed_native_file(root))
        .cloned()
        .collect();
    retire_other_sources(storage, &roots, "zed")
}

fn zed_native_file(path: &Path) -> bool {
    // A filename alone never hides a manually selected corrupt database.
    path.is_file()
        && path.file_name().and_then(|s| s.to_str()) == Some("threads.db")
        && matches!(
            super::zed::detect::detect(path),
            Ok(DetectOutcome::Supported { .. })
        )
}

fn retire_other_sources(storage: &Storage, roots: &[PathBuf], keep: &str) -> Result<(), CoreError> {
    // Old discovery could transform a supplied root (Junie lifts a session
    // directory to its parent). Reproduce that metadata path through the registry
    // instead of assuming every adapter registered the literal supplied path.
    // No home/env roots: only the old broadcast's managed inputs are candidates.
    let legacy_context = DiscoverContext {
        manual_roots: roots.to_vec(),
        ..Default::default()
    };
    let mut candidates = std::collections::BTreeSet::new();
    for adapter in super::built_in_adapters() {
        if adapter.adapter_id() == keep {
            continue;
        }
        for root in roots {
            candidates.insert((adapter.adapter_id(), normalize_path(root)));
        }
        for discovered in adapter.discover(&legacy_context) {
            candidates.insert((adapter.adapter_id(), normalize_path(&discovered.root)));
        }
    }
    for (adapter, path) in candidates {
        storage.conn().execute(
            "UPDATE source_instances SET health='not_applicable'
             WHERE location_hint=?1 AND format=?2
               AND NOT EXISTS(SELECT 1 FROM usage_events e WHERE e.source_instance_id=source_instances.instance_id)
               AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.instance_id=source_instances.instance_id)
               AND NOT EXISTS(SELECT 1 FROM period_usage p WHERE p.instance_id=source_instances.instance_id)
               AND NOT EXISTS(SELECT 1 FROM source_aggregates a WHERE a.instance_id=source_instances.instance_id)",
            params![path,adapter],
        )?;
    }
    Ok(())
}

fn qwen_record(path: &Path) -> bool {
    let limits = super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: 1024 * 1024,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_millis(200)),
    };
    let Ok(outcome) = super::jsonl::read_jsonl(path, 0, 1, &limits) else {
        return false;
    };
    let Some(line) = outcome.lines.first() else {
        return false;
    };
    let Ok(value) = super::run_policy::json_from_str::<serde_json::Value>(&line.text) else {
        return false;
    };
    ["uuid", "sessionId", "timestamp"]
        .iter()
        .all(|key| value[*key].as_str().is_some_and(|v| !v.is_empty()))
        && super::qwen::versions::chatrecord_085e98c0::RECORD_TYPES
            .contains(&value["type"].as_str().unwrap_or_default())
        && (value["usageMetadata"].is_object()
            || (value["message"]["parts"].is_array()
                && value["message"]["role"].is_string()
                && value["message"].get("content").is_none()))
}

fn qwen_native_root(root: &Path) -> bool {
    if root.file_name().and_then(|s| s.to_str()) != Some(".qwen") {
        return false;
    }
    let ctx = DiscoverContext {
        manual_roots: vec![root.to_path_buf()],
        ..Default::default()
    };
    super::qwen::QwenAdapter::new()
        .discover(&ctx)
        .iter()
        .flat_map(|r| &r.files)
        .take(8)
        .any(|p| qwen_record(p))
}

fn recoverable_qwen_owner(
    storage: &Storage,
    instance: &str,
    owner: &str,
    path: &Path,
) -> Result<bool, CoreError> {
    if !instance.starts_with("qwen@")
        || !qwen_record(path)
        || !path
            .ancestors()
            .skip(1)
            .take(5)
            .any(|p| p.file_name().and_then(|s| s.to_str()) == Some(".qwen"))
    {
        return Ok(false);
    }
    storage.conn().query_row("SELECT NOT EXISTS(SELECT 1 FROM usage_events WHERE source_instance_id=?1) AND NOT EXISTS(SELECT 1 FROM daily_usage WHERE instance_id=?1) AND NOT EXISTS(SELECT 1 FROM period_usage WHERE instance_id=?1) AND NOT EXISTS(SELECT 1 FROM source_aggregates WHERE instance_id=?1)",[owner],|r|r.get(0)).map_err(Into::into)
}

pub(super) fn accepts_manual_file(
    storage: &Storage,
    instance: &str,
    path: &Path,
) -> Result<bool, CoreError> {
    // Exact DSH framing is stronger evidence than another reader's shared
    // `type=session` word. Corrupt manual files still follow normal diagnostics.
    if !instance.starts_with("dsh@") && dsh_native_record(path) {
        return Ok(false);
    }
    let owner: Option<String> = storage
        .conn()
        .query_row(
            "SELECT instance_id FROM source_files WHERE instance_id!=?1 AND file_id=?2
          AND (format_status IS NOT NULL OR status!='unsupported')",
            params![instance, normalize_path(path)],
            |r| r.get(0),
        )
        .optional()?;
    // Unknown user-selected files still need diagnostics. Only the app's managed
    // roots have an explicit adapter route; don't silently hide genuine corruption.
    match owner {
        None => Ok(true),
        Some(owner) => Ok(recoverable_qwen_owner(storage, instance, &owner, path)?
            || recoverable_dsh_owner(storage, instance, &owner, path)?),
    }
}

fn dsh_native_record(path: &Path) -> bool {
    if path.extension().is_some_and(|v| v == "zstd") {
        return super::dsh::versions::session_v4::generation(path) == Some(4)
            && matches!(
                super::dsh::versions::session_v4::detect(path),
                Ok(DetectOutcome::Supported { .. })
            );
    }
    if !path.extension().is_some_and(|v| v == "jsonl") {
        return false;
    }
    let limits = super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: 4 * 1024 * 1024,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_millis(200)),
    };
    let Ok(result) = super::jsonl::read_jsonl(path, 0, 1, &limits) else {
        return false;
    };
    result
        .lines
        .first()
        .and_then(|line| super::run_policy::json_from_str::<serde_json::Value>(&line.text).ok())
        .is_some_and(|doc| super::dsh::versions::session_v4::header(&doc))
}

fn recoverable_dsh_owner(
    storage: &Storage,
    instance: &str,
    owner: &str,
    path: &Path,
) -> Result<bool, CoreError> {
    if !instance.starts_with("dsh@") || !dsh_native_record(path) {
        return Ok(false);
    }
    storage.conn().query_row("SELECT NOT EXISTS(SELECT 1 FROM usage_events WHERE source_instance_id=?1) AND NOT EXISTS(SELECT 1 FROM daily_usage WHERE instance_id=?1) AND NOT EXISTS(SELECT 1 FROM period_usage WHERE instance_id=?1) AND NOT EXISTS(SELECT 1 FROM source_aggregates WHERE instance_id=?1)",[owner],|r|r.get(0)).map_err(Into::into)
}

/// A physical file has one owner. Recover only an unrecognized registration
/// that never committed usage or a cursor; never transfer a valid stream.
pub(super) fn claim_file(
    storage: &Storage,
    adapter: &dyn SourceAdapter,
    instance: &str,
    path: &Path,
) -> Result<bool, CoreError> {
    let file = normalize_path(path);
    let owner: Option<(String, bool)> = storage
        .conn()
        .query_row(
            "SELECT instance_id, status='unsupported' AND format_status IS NULL
         FROM source_files WHERE file_id=?1",
            [&file],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((owner, unrecognized)) = owner else {
        return Ok(true);
    };
    if owner == instance {
        return Ok(true);
    }
    if recoverable_dsh_owner(storage, instance, &owner, path)? {
        let tx = storage.conn().unchecked_transaction()?;
        tx.execute("DELETE FROM ingestion_checkpoints WHERE instance_id=?1 AND scope_key=(SELECT file_identity FROM source_files WHERE file_id=?2 AND instance_id=?1)",params![owner,file])?;
        let changed=tx.execute("UPDATE source_files SET instance_id=?1,status='new',format_status=NULL WHERE file_id=?2 AND instance_id=?3",params![instance,file,owner])?;
        if changed == 1 {
            tx.execute("UPDATE source_instances SET health='not_applicable' WHERE instance_id=?1 AND NOT EXISTS(SELECT 1 FROM source_files WHERE instance_id=?1)",[&owner])?;
        }
        tx.commit()?;
        return Ok(changed == 1);
    }
    if recoverable_qwen_owner(storage, instance, &owner, path)? {
        let tx = storage.conn().unchecked_transaction()?;
        tx.execute("DELETE FROM ingestion_checkpoints WHERE instance_id=?1 AND scope_key=(SELECT file_identity FROM source_files WHERE file_id=?2 AND instance_id=?1)",params![owner,file])?;
        let changed=tx.execute("UPDATE source_files SET instance_id=?1,status='new',format_status=NULL WHERE file_id=?2 AND instance_id=?3",params![instance,file,owner])?;
        tx.execute(
            "UPDATE source_instances SET health='not_applicable' WHERE instance_id=?1",
            [owner],
        )?;
        tx.commit()?;
        return Ok(changed == 1);
    }
    if !unrecognized || !matches!(adapter.detect(path)?, DetectOutcome::Supported { .. }) {
        return Ok(false);
    }
    let tx = storage.conn().unchecked_transaction()?;
    let changed = tx.execute(
        "UPDATE source_files SET instance_id=?1, status='new', format_status=NULL
         WHERE file_id=?2 AND instance_id=?3
           AND NOT EXISTS(SELECT 1 FROM ingestion_checkpoints c WHERE c.instance_id=?3 AND c.scope_key=source_files.file_identity)
           AND NOT EXISTS(SELECT 1 FROM usage_events e WHERE e.source_instance_id=?3)
           AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.instance_id=?3)
           AND NOT EXISTS(SELECT 1 FROM period_usage p WHERE p.instance_id=?3)
           AND NOT EXISTS(SELECT 1 FROM source_aggregates a WHERE a.instance_id=?3)",
        params![instance, file, owner],
    )?;
    if changed == 1 {
        // Only an empty unrecognized owner whose last file was transferred.
        // Other manually selected bad files still retain their visible diagnosis.
        tx.execute("UPDATE source_instances SET health='not_applicable' WHERE instance_id=?1 AND NOT EXISTS(SELECT 1 FROM source_files WHERE instance_id=?1)",[&owner])?;
    }
    tx.commit()?;
    Ok(changed == 1)
}
