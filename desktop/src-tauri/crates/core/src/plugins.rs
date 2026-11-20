use crate::{model_aliases, storage::Storage, CoreError};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAX_COMPRESSED: usize = 2 * 1024 * 1024;
const MAX_MANIFEST: usize = 16 * 1024;
const MAX_EXPANDED: usize = model_aliases::MAX_CATALOG_BYTES + MAX_MANIFEST + 8192;
const MAX_INSTALLED: i64 = 32;
const MAX_HISTORY: i64 = 1024;
const MAX_HISTORY_BYTES: i64 = 32 * 1024 * 1024;
const MAX_ACTIVE: usize = 8;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub api_major: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub publisher: String,
    pub license: String,
    pub contribution: Contribution,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub id: String,
    pub kind: String,
    pub entry: String,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub manifest: Manifest,
    pub digest: String,
    pub rule_version: String,
    pub rule_count: u32,
    pub enabled: bool,
}

pub struct Package {
    info: PackageInfo,
    raw_manifest: String,
    aliases: String,
}

fn invalid(reason: &str) -> CoreError {
    CoreError::Validation(reason.to_owned())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn plugin_id(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && !value.starts_with("llm-usage.")
        && !value.starts_with("builtin.")
        && value.split('.').all(|part| {
            identifier(part)
                && !matches!(part, "con" | "prn" | "aux" | "nul")
                && !["com", "lpt"].iter().any(|prefix| {
                    part.strip_prefix(prefix)
                        .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
                })
        })
}

fn display_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn manifest(raw: &[u8]) -> Result<Manifest, CoreError> {
    let result: Manifest = serde_json::from_slice(raw).map_err(|_| invalid("invalid_manifest"))?;
    if result.format != "llm-usage-plugin/1"
        || result.api_major != 1
        || result.contribution.kind != "model_aliases"
        || result.contribution.entry != "aliases.json"
    {
        return Err(invalid("unsupported_manifest"));
    }
    if !plugin_id(&result.id)
        || result.version.len() > 64
        || semver::Version::parse(&result.version).is_err()
        || !identifier(&result.contribution.id)
        || !display_text(&result.name)
        || !display_text(&result.publisher)
        || !display_text(&result.license)
    {
        return Err(invalid("invalid_manifest"));
    }
    Ok(result)
}

fn checked_catalog(
    raw: &str,
    expected: &Manifest,
    now_ms: i64,
) -> Result<model_aliases::Catalog, CoreError> {
    if model_aliases::digest(raw.as_bytes()) != expected.contribution.sha256 {
        return Err(invalid("asset_digest_mismatch"));
    }
    let catalog = model_aliases::parse_catalog(raw, now_ms)?;
    if catalog.aliases.iter().any(|rule| rule.basis != "catalogue") {
        return Err(invalid("invalid_plugin_alias_basis"));
    }
    Ok(catalog)
}

impl Package {
    pub fn open(path: &Path, now_ms: i64) -> Result<Self, CoreError> {
        if !fs::metadata(path)?.is_file() {
            return Err(invalid("invalid_archive"));
        }
        let bytes = read_bounded(File::open(path)?, MAX_COMPRESSED)?;
        Self::decode(&bytes, now_ms)
    }

    pub fn decode(bytes: &[u8], now_ms: i64) -> Result<Self, CoreError> {
        if bytes.len() > MAX_COMPRESSED {
            return Err(invalid("package_size_limit"));
        }
        let mut decoder =
            zstd::stream::read::Decoder::new(bytes).map_err(|_| invalid("invalid_archive"))?;
        decoder
            .window_log_max(21)
            .map_err(|_| invalid("invalid_archive"))?;
        let decoded = read_bounded(decoder, MAX_EXPANDED)?;
        let mut archive = tar::Archive::new(decoded.as_slice());
        archive.set_ignore_zeros(true);
        let mut manifest_bytes = None;
        let mut aliases_bytes = None;
        for entry in archive
            .entries()
            .map_err(|_| invalid("invalid_archive"))?
            .raw(true)
        {
            let entry = entry.map_err(|_| invalid("invalid_archive"))?;
            if !entry.header().entry_type().is_file() {
                return Err(invalid("invalid_archive_member"));
            }
            let (target, limit) = match entry.path_bytes().as_ref() {
                b"manifest.json" => (&mut manifest_bytes, MAX_MANIFEST),
                b"aliases.json" => (&mut aliases_bytes, model_aliases::MAX_CATALOG_BYTES),
                _ => return Err(invalid("invalid_archive_member")),
            };
            if target.is_some() {
                return Err(invalid("duplicate_archive_member"));
            }
            *target = Some(read_bounded(entry, limit)?);
        }
        let raw_manifest =
            String::from_utf8(manifest_bytes.ok_or_else(|| invalid("missing_manifest"))?)
                .map_err(|_| invalid("invalid_manifest"))?;
        let manifest = manifest(raw_manifest.as_bytes())?;
        let aliases = String::from_utf8(aliases_bytes.ok_or_else(|| invalid("missing_aliases"))?)
            .map_err(|_| invalid("invalid_alias_utf8"))?;
        let catalog = checked_catalog(&aliases, &manifest, now_ms)?;
        Ok(Self {
            info: PackageInfo {
                manifest,
                digest: model_aliases::digest(raw_manifest.as_bytes()),
                rule_version: catalog.rule_version,
                rule_count: catalog.aliases.len() as u32,
                enabled: false,
            },
            raw_manifest,
            aliases,
        })
    }

    pub fn info(&self) -> &PackageInfo {
        &self.info
    }
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, CoreError> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(invalid("package_size_limit"));
    }
    Ok(bytes)
}

pub(crate) fn install_schema(storage: &Storage) -> Result<(), CoreError> {
    storage.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS desktop_plugin_versions_v1 (
            id TEXT NOT NULL,
            version TEXT NOT NULL,
            digest TEXT NOT NULL,
            manifest TEXT NOT NULL,
            aliases TEXT NOT NULL,
            rule_version TEXT NOT NULL,
            rule_count INTEGER NOT NULL,
            installed INTEGER NOT NULL,
            PRIMARY KEY (id, version)
        )",
    )?;
    Ok(())
}

pub fn install(storage: &Storage, package: &Package) -> Result<(), CoreError> {
    let info = &package.info;
    let tx = storage.conn().unchecked_transaction()?;
    let previous: Option<(String, bool)> = tx
        .query_row(
            "SELECT digest, installed FROM desktop_plugin_versions_v1 WHERE id=?1 AND version=?2",
            params![info.manifest.id, info.manifest.version],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((digest, installed)) = &previous {
        if digest != &info.digest {
            return Err(invalid("package_version_conflict"));
        }
        if *installed {
            return Ok(());
        }
    }
    let (history, installed, bytes): (i64, i64, i64) = tx.query_row(
        "SELECT count(*), coalesce(sum(installed), 0),
         coalesce(sum(length(CAST(aliases AS BLOB)) + length(CAST(manifest AS BLOB))), 0)
         FROM desktop_plugin_versions_v1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let additional_bytes = (package.aliases.len() + package.raw_manifest.len()) as i64;
    if installed >= MAX_INSTALLED
        || (previous.is_none()
            && (history >= MAX_HISTORY
                || bytes.saturating_add(additional_bytes) > MAX_HISTORY_BYTES))
    {
        return Err(invalid("package_history_limit"));
    }
    tx.execute(
        "INSERT INTO desktop_plugin_versions_v1
         (id, version, digest, manifest, aliases, rule_version, rule_count, installed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
         ON CONFLICT(id, version) DO UPDATE SET installed=1",
        params![
            info.manifest.id,
            info.manifest.version,
            info.digest,
            package.raw_manifest,
            package.aliases,
            info.rule_version,
            info.rule_count
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn receipt_path(storage: &Storage, id: &str) -> Result<PathBuf, CoreError> {
    if !plugin_id(id) {
        return Err(invalid("invalid_plugin_id"));
    }
    let db = storage
        .path()
        .file_name()
        .ok_or_else(|| invalid("missing_database_path"))?;
    let mut dir = db.to_os_string();
    dir.push(".plugin-grants-v1");
    Ok(storage
        .path()
        .with_file_name(dir)
        .join(format!("{id}.grant")))
}

fn active(storage: &Storage, id: &str) -> Option<String> {
    let path = receipt_path(storage, id).ok()?;
    if !fs::symlink_metadata(&path).ok()?.is_file() {
        return None;
    }
    let value = read_bounded(File::open(path).ok()?, 64).ok()?;
    String::from_utf8(value).ok()
}

pub fn list(storage: &Storage) -> Result<Vec<PackageInfo>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT id, version, digest, manifest, rule_version, rule_count
         FROM desktop_plugin_versions_v1 WHERE installed=1 ORDER BY id, version",
    )?;
    let mut rows = stmt.query([])?;
    let mut result = Vec::new();
    let mut receipts = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let version: String = row.get(1)?;
        let digest: String = row.get(2)?;
        let raw: String = row.get(3)?;
        let manifest = manifest(raw.as_bytes())?;
        if manifest.id != id
            || manifest.version != version
            || model_aliases::digest(raw.as_bytes()) != digest
        {
            return Err(invalid("stored_manifest_digest_mismatch"));
        }
        let enabled = receipts
            .entry(id.clone())
            .or_insert_with(|| active(storage, &id));
        result.push(PackageInfo {
            manifest,
            enabled: enabled.as_ref() == Some(&digest),
            digest,
            rule_version: row.get(4)?,
            rule_count: row.get(5)?,
        });
    }
    Ok(result)
}

pub fn set_enabled(
    storage: &Storage,
    id: &str,
    version: &str,
    digest: &str,
    enabled: bool,
) -> Result<(), CoreError> {
    let path = receipt_path(storage, id)?;
    let installed = list(storage)?;
    let item = installed
        .iter()
        .find(|item| {
            item.manifest.id == id && item.manifest.version == version && item.digest == digest
        })
        .ok_or_else(|| invalid("package_not_installed"))?;
    if item.enabled == enabled {
        return Ok(());
    }
    if !enabled {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    load_catalog(storage, &item.manifest, digest)?;
    if installed.iter().filter(|p| p.enabled).count() >= MAX_ACTIVE
        && !installed.iter().any(|p| p.enabled && p.manifest.id == id)
    {
        return Err(invalid("active_package_limit"));
    }
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| invalid("missing_receipt_directory"))?,
    )?;
    let staged = path.with_extension(format!("{}.pending", crate::jobs::now_ms_fallback()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    let result = (|| -> io::Result<()> {
        file.write_all(digest.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&staged, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(staged);
    }
    Ok(result?)
}

pub fn remove(storage: &Storage, id: &str, version: &str, digest: &str) -> Result<(), CoreError> {
    set_enabled(storage, id, version, digest, false)?;
    storage.conn().execute(
        "UPDATE desktop_plugin_versions_v1 SET installed=0
         WHERE id=?1 AND version=?2 AND digest=?3",
        params![id, version, digest],
    )?;
    Ok(())
}

pub(crate) fn active_versions(storage: &Storage) -> Vec<(Manifest, String)> {
    list(storage)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.enabled)
        .take(MAX_ACTIVE)
        .map(|item| (item.manifest, item.digest))
        .collect()
}

pub(crate) fn load_catalog(
    storage: &Storage,
    manifest: &Manifest,
    digest: &str,
) -> Result<model_aliases::Catalog, CoreError> {
    let aliases: String = storage.conn().query_row(
        "SELECT aliases FROM desktop_plugin_versions_v1
         WHERE id=?1 AND version=?2 AND digest=?3 AND installed=1",
        params![manifest.id, manifest.version, digest],
        |row| row.get(0),
    )?;
    checked_catalog(&aliases, manifest, i64::MAX)
}
