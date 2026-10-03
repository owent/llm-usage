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
    if adapter_id == "otel" {
        ctx.manual_roots.extend_from_slice(copilot_otel_roots);
    }
    ctx
}

/// Keep valid history and settings. Only hide empty instances created by the old
/// managed-root broadcast; a future real discovery makes them active again.
pub fn retire_misrouted_sources(storage: &Storage, roots: &[PathBuf]) -> Result<(), CoreError> {
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
        if adapter.adapter_id() == "otel" {
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

pub(super) fn accepts_manual_file(
    storage: &Storage,
    instance: &str,
    path: &Path,
) -> Result<bool, CoreError> {
    let owned_elsewhere: bool = storage.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM source_files WHERE instance_id!=?1 AND file_id=?2
         AND (format_status IS NOT NULL OR status!='unsupported'))",
        params![instance, normalize_path(path)],
        |r| r.get(0),
    )?;
    // Unknown user-selected files still need diagnostics. Only the app's managed
    // roots have an explicit adapter route; don't silently hide genuine corruption.
    Ok(!owned_elsewhere)
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
    if !unrecognized || !matches!(adapter.detect(path)?, DetectOutcome::Supported { .. }) {
        return Ok(false);
    }
    let changed = storage.conn().execute(
        "UPDATE source_files SET instance_id=?1, status='new', format_status=NULL
         WHERE file_id=?2 AND instance_id=?3
           AND NOT EXISTS(SELECT 1 FROM ingestion_checkpoints c WHERE c.instance_id=?3 AND c.scope_key=source_files.file_identity)
           AND NOT EXISTS(SELECT 1 FROM usage_events e WHERE e.source_instance_id=?3)
           AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.instance_id=?3)
           AND NOT EXISTS(SELECT 1 FROM period_usage p WHERE p.instance_id=?3)
           AND NOT EXISTS(SELECT 1 FROM source_aggregates a WHERE a.instance_id=?3)",
        params![instance, file, owner],
    )?;
    Ok(changed == 1)
}
