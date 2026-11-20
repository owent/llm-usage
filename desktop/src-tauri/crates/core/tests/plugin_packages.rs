mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::WeekStart;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::model_aliases::{self, Book};
use llm_usage_core::plugins::{self, Package};
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::storage::Storage;
use serde_json::json;
use tar::{Builder, Header};

fn append(archive: &mut Builder<Vec<u8>>, name: &str, data: &[u8]) {
    let mut header = Header::new_ustar();
    header.set_mode(0o644);
    header.set_size(data.len() as u64);
    archive.append_data(&mut header, name, data).unwrap();
}

fn archive(id: &str, version: &str, label: &str, extra: Option<&str>) -> Vec<u8> {
    let aliases = json!({
        "schema_version": 1,
        "rule_version": "2026-11",
        "published_at": "2026-11-16T00:00:00Z",
        "aliases": [{
            "agent": "codex",
            "key": "plugin-sample",
            "display_name": label,
            "basis": "catalogue",
            "effective_from": "2026-11-16T00:00:00Z",
        }],
    })
    .to_string();
    let manifest = json!({
        "format": "llm-usage-plugin/1",
        "api_major": 1,
        "id": id,
        "version": version,
        "name": "Offline sample",
        "publisher": "Plugin tests",
        "license": "MIT",
        "contribution": {
            "id": "models",
            "kind": "model_aliases",
            "entry": "aliases.json",
            "sha256": model_aliases::digest(aliases.as_bytes()),
        },
    })
    .to_string();
    let mut builder = Builder::new(Vec::new());
    append(&mut builder, "manifest.json", manifest.as_bytes());
    append(&mut builder, "aliases.json", aliases.as_bytes());
    if let Some(extra) = extra {
        append(&mut builder, extra, b"extra");
    }
    zstd::stream::encode_all(builder.into_inner().unwrap().as_slice(), 1).unwrap()
}

fn package(id: &str, version: &str, label: &str) -> Package {
    Package::decode(
        &archive(id, version, label, None),
        ts("2026-11-17T00:00:00Z"),
    )
    .unwrap()
}

fn enable(storage: &Storage, pack: &Package, enabled: bool) {
    let info = pack.info();
    plugins::set_enabled(
        storage,
        &info.manifest.id,
        &info.manifest.version,
        &info.digest,
        enabled,
    )
    .unwrap();
}

#[test]
fn package_lifecycle_updates_existing_queries_and_ignores_grants_in_a_database_copy() {
    let (dir, storage) = temp_storage("plugin-lifecycle");
    let at = ts("2026-11-16T10:00:00Z");
    let day = jiff::civil::date(2026, 11, 16);
    let mut event = with_tokens(evt("source", "call", at), 100, 20);
    event.agent = "codex".into();
    event.model_raw = Some("plugin-sample".into());
    commit_batch(&storage, &batch("source", "UTC", at, vec![event]), None).unwrap();
    let query = SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: day,
        last_day: day,
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: day,
        retention_cutoff: None,
    };
    let baseline = query_summary(&storage, &query).unwrap();
    let path = storage.path().to_owned();
    storage
        .conn()
        .execute("DROP TABLE desktop_plugin_versions_v1", [])
        .unwrap();
    drop(storage);
    let storage = Storage::open(&path).unwrap();
    assert_eq!(
        query_summary(&storage, &query).unwrap().totals,
        baseline.totals
    );
    assert!(plugins::list(&storage).unwrap().is_empty());
    let first = package("example.labels", "1.0.0", "First display label");
    let second = package("example.labels", "1.1.0", "Second display label");
    plugins::install(&storage, &first).unwrap();
    plugins::install(&storage, &first).unwrap();
    assert_eq!(plugins::list(&storage).unwrap().len(), 1);
    assert!(!plugins::list(&storage).unwrap()[0].enabled);
    assert!(plugins::install(&storage, &package("example.labels", "1.0.0", "Changed")).is_err());
    enable(&storage, &first, true);
    let updated = query_summary(&storage, &query).unwrap();
    assert_ne!(updated.model_alias_revision, baseline.model_alias_revision);
    assert_eq!(updated.data_revision, baseline.data_revision);
    assert_eq!(updated.totals, baseline.totals);
    assert_eq!(
        updated.model_breakdown[0].display_model.as_deref(),
        Some("First display label")
    );
    assert_eq!(
        updated.model_breakdown[0].model_raw.as_deref(),
        Some("plugin-sample")
    );
    plugins::install(&storage, &second).unwrap();
    enable(&storage, &second, true);
    drop(storage);
    let storage = Storage::open(&path).unwrap();
    let summary = query_summary(&storage, &query).unwrap();
    assert_eq!(
        summary.model_breakdown[0].display_model.as_deref(),
        Some("Second display label")
    );
    let backup = dir.path().join("backup.sqlite3");
    storage
        .conn()
        .execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])
        .unwrap();
    let copy = Storage::open(&backup).unwrap();
    assert!(!plugins::list(&copy).unwrap().iter().any(|row| row.enabled));
    enable(&storage, &first, true);
    let info = first.info();
    plugins::remove(
        &storage,
        &info.manifest.id,
        &info.manifest.version,
        &info.digest,
    )
    .unwrap();
    assert_eq!(plugins::list(&storage).unwrap().len(), 1);
    let retained: String = storage
        .conn()
        .query_row(
            "SELECT aliases FROM desktop_plugin_versions_v1 WHERE id=?1 AND version=?2",
            [&info.manifest.id, &info.manifest.version],
            |row| row.get(0),
        )
        .unwrap();
    assert!(retained.contains("First display label"));
    assert_eq!(
        query_summary(&storage, &query)
            .unwrap()
            .model_alias_revision,
        baseline.model_alias_revision
    );
    plugins::install(&storage, &first).unwrap();
    assert!(!plugins::list(&storage)
        .unwrap()
        .iter()
        .any(|row| row.enabled));
    assert_eq!(storage.schema_version().unwrap(), 11);
    drop(storage);
    drop(copy);
    std::fs::copy(&backup, &path).unwrap();
    let restored = Storage::open(&path).unwrap();
    assert!(!plugins::list(&restored)
        .unwrap()
        .iter()
        .any(|row| row.enabled));
    assert_eq!(
        query_summary(&restored, &query).unwrap().totals,
        baseline.totals
    );
}

#[test]
fn extra_or_duplicate_members_and_reserved_identity_are_rejected() {
    let now = ts("2026-11-17T00:00:00Z");
    for extra in ["aliases.json", "unlisted.json", "ALIASES.JSON"] {
        assert!(Package::decode(
            &archive("example.labels", "1.0.0", "Demo", Some(extra)),
            now
        )
        .is_err());
    }
    assert!(Package::decode(&archive("builtin.labels", "1.0.0", "Demo", None), now).is_err());
    assert!(Package::decode(b"not an archive", now).is_err());
    let mut bytes = archive("example.labels", "1.0.0", "Demo", None);
    bytes.truncate(bytes.len() - 5);
    assert!(Package::decode(&bytes, now).is_err());
}

#[test]
fn equal_rank_plugins_keep_conflicts_and_manual_names_win() {
    let (_dir, storage) = temp_storage("plugin-conflict");
    let alpha = package("example.alpha", "1.0.0", "Alpha");
    let beta = package("example.beta", "1.0.0", "Beta");
    for pack in [&alpha, &beta] {
        plugins::install(&storage, pack).unwrap();
        enable(&storage, pack, true);
    }
    let at = ts("2026-11-16T10:00:00Z");
    let label = || Book::load(&storage).label("plugin-sample", "codex", None, at, at + 1);
    assert_eq!(label(), "plugin-sample");
    enable(&storage, &beta, false);
    assert_eq!(label(), "Alpha");
    let raw = json!({
        "schema_version": 1,
        "rule_version": "2026-11",
        "published_at": "2026-11-16T00:00:00Z",
        "aliases": [{
            "agent": "codex",
            "key": "plugin-sample",
            "display_name": "Manual label",
            "basis": "user_override",
            "effective_from": "2026-11-16T00:00:00Z",
        }],
    });
    model_aliases::import_catalog(&storage, &raw.to_string(), at, "manual", None, false).unwrap();
    assert_eq!(label(), "Manual label");
}
