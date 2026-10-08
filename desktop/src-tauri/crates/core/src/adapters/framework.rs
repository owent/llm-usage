//! Common adapter interface: discover, detect, scan, map and capability,
//! with a runner feeding M1 commit_batch. V12: repeated reads do not duplicate usage.
//!
//! Rules:
//! - discover: bounded candidate paths, environment overrides and manual roots; no full-disk scan;
//! - detect: file signatures, record types and schema fingerprints, with per-Agent version registries;
//!   unknown/missing versions try the latest built-in Agent reader with compatibility metadata (V17/V30);
//!   verified incompatible versions and unknown formats remain restricted;
//! - scan: incremental JSONL cursors keep file identity, generation, complete-line byte offset and parse context;
//! - capability: declare the fields available from each source;
//! - diagnostics keep field names, error codes and positions without copying raw content.

use crate::domain::EventInput;
use crate::error::CoreError;
use crate::ingest::{self, BatchOutcome, CheckpointUpdate, DiagnosticInput, IngestBatch};
use crate::jobs::{self, RunStart, RunStats, RunStatus, TriggerKind};
use crate::storage::Storage;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::jsonl::{FileProbe, JsonlLimits, StoredFileState};

/// Default wall-clock timeout for one source run: 30 seconds (architecture.md).
pub const DEFAULT_SOURCE_TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
/// Discovery file/directory limits.
pub const DISCOVER_MAX_FILES: usize = 20_000;
pub const DISCOVER_MAX_DIRS: usize = 50_000;

/// Transient detection I/O: short antivirus/product locks, Windows sharing violations
/// mapped to PermissionDenied, timeouts or files deleted after enumeration.
/// Return Pending and retry next run instead of persisting a scan failure or UnknownFormat.
pub fn is_transient_io(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::NotFound
    )
}

/// Read a detection header; Ok(None) means transiently unreadable and requires Pending.
/// Propagate other errors as CoreError.
pub fn read_detect_head(path: &Path, head_bytes: usize) -> Result<Option<Vec<u8>>, CoreError> {
    use std::io::Read as _;
    let file = match super::run_policy::checked_file(path) {
        Ok(file) => file,
        Err(err) if is_transient_io(&err) => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut head = Vec::with_capacity(head_bytes);
    match file.take(head_bytes as u64).read_to_end(&mut head) {
        Ok(_) => {}
        Err(err) if is_transient_io(&err) => return Ok(None),
        Err(err) => return Err(err.into()),
    }
    Ok(Some(head))
}

/// Discovery context: an explicit environment map supports isolated tests without process-environment reads.
#[derive(Debug, Clone, Default)]
pub struct DiscoverContext {
    pub home_dir: Option<PathBuf>,
    pub env: std::collections::BTreeMap<String, String>,
    pub manual_roots: Vec<PathBuf>,
}

/// How a candidate root was selected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootBasis {
    /// Environment override; the stored string is the variable name, not its value.
    EnvOverride(String),
    /// Platform default user directory.
    DefaultHome,
    /// User-added manual root.
    Manual,
}

/// Candidate root with bounded file enumeration.
#[derive(Debug, Clone)]
pub struct DiscoveredRoot {
    pub root: PathBuf,
    pub basis: RootBasis,
    pub files: Vec<PathBuf>,
}

/// Detection results follow architecture.md compatibility rules:
/// - Supported: verified Agent identity/input type; known versions dispatch through the registry.
///   Unknown/missing versions select LatestFallback and retain compatibility metadata.
/// - UnsupportedVersion: verified incompatibility, such as a different source format; no fallback.
/// - UnknownFormat: unverified Agent/format stays restricted instead of reporting successful zero records (V17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectOutcome {
    Supported {
        format: String,
        /// Native version, or None when absent; LatestFallback remains possible.
        format_version: Option<String>,
        basis: crate::domain::VersionBasis,
    },
    UnsupportedVersion {
        format: String,
        found: Option<String>,
        reason: String,
    },
    UnknownFormat {
        reason: String,
    },
    /// No classifiable content yet, such as an empty new file; retry next run.
    Pending,
}

/// Availability of one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Partial(String),
    Unavailable(String),
}

/// Structured discovery, detection, mapping, incremental, deduplication, coverage, maintenance and scheduling capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityTable {
    pub adapter_id: String,
    pub product: String,
    pub surfaces: Vec<String>,
    pub supported_versions: Vec<String>,
    pub discovery: serde_json::Value,
    pub detection: serde_json::Value,
    /// Token/cache/call/model/time/cost/latency field availability.
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub lifecycle: serde_json::Value,
    pub incremental: serde_json::Value,
    pub dedup: serde_json::Value,
    pub integrity: serde_json::Value,
    pub maintenance: serde_json::Value,
    pub scheduling: serde_json::Value,
    pub limitations: Vec<String>,
}

/// One file to scan.
#[derive(Debug, Clone)]
pub struct ScanTarget {
    pub instance_id: String,
    /// Normalized local file path; store locally and exclude from external output.
    pub path: PathBuf,
    /// source_files.file_id, based on the normalized path.
    pub file_id: String,
    /// source_files.file_identity/checkpoint scope_key, stable across supported renames.
    pub file_identity: String,
    /// Current observed file state.
    pub probe: FileProbe,
    /// Generation selected by file-continuity rules; a rescan already uses the new value.
    pub generation: i64,
    /// Whether replacement, truncation or rebuilding requires a scan from the beginning.
    pub rescan: bool,
}

/// Stored cursor and parse context.
#[derive(Debug, Clone, Default)]
pub struct StoredScanState {
    pub cursor: Option<serde_json::Value>,
    pub parse_context: Option<serde_json::Value>,
}

/// Scan limits.
#[derive(Debug, Clone, Default)]
pub struct ScanLimits {
    pub jsonl: JsonlLimits,
}

/// File scan status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanStatus {
    /// Reached the current end of file.
    Complete,
    /// Read/time limit reached; preserve a complete-line cursor and continue next run.
    BudgetExhausted,
    /// Oversized line: restrict reading at its start and permit a controlled retry.
    LineTooLong,
    /// Empty file awaiting content.
    Pending,
}

impl ScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanStatus::Complete => "complete",
            ScanStatus::BudgetExhausted => "budget_exhausted",
            ScanStatus::LineTooLong => "line_too_long",
            ScanStatus::Pending => "pending",
        }
    }
}

/// Snapshot reconciliation with selected numeric fields, without message content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconciliation {
    /// Series label, such as session_cumulative_snapshot, without IDs/paths.
    pub series: String,
    /// Sum of detailed total_tokens values.
    pub detail_sum: i64,
    /// Final cumulative snapshot value.
    pub snapshot_final: Option<i64>,
    /// Sum of boundary records excluded by verified rules, such as compaction-carried usage.
    pub carried_sum: i64,
    /// Difference: detail_sum - (snapshot_final + carried_sum).
    pub difference: Option<i64>,
    /// Reconciliation verdict: matched, mismatch or no_snapshot.
    pub verdict: String,
}

/// One scan result.
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    pub status: ScanStatus,
    pub cursor: Option<serde_json::Value>,
    pub parse_context: Option<serde_json::Value>,
    pub events: Vec<EventInput>,
    pub aggregates: Vec<crate::aggregates::SourceAggregateInput>,
    pub diagnostics: Vec<DiagnosticInput>,
    pub lines_read: u64,
    pub records_seen: u64,
    pub reconciliations: Vec<Reconciliation>,
    /// Source-file health: active or degraded.
    pub health: String,
}

/// Common adapter interface.
pub trait SourceAdapter: Send + Sync {
    fn adapter_id(&self) -> &'static str;
    /// Agent name used by statistics and event.agent.
    fn agent(&self) -> &'static str;
    /// Bounded discovery through candidate paths, environment overrides and manual roots.
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot>;
    /// Opt in only when files can be visited independently of discovery order.
    fn rotate_file_windows(&self) -> bool {
        false
    }
    /// Stable local instance ID for a candidate root.
    fn instance_id(&self, root: &DiscoveredRoot) -> String;
    /// Detect file signature/record type/schema; use the per-Agent registry for version selection.
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError>;
    /// Incrementally scan one supported file.
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError>;
    /// Recheck consumed, unchanged files while preserving monotonic revisions in cursor context.
    /// Skip by default; snapshot adapters can opt in for a one-time statistics/health rule correction.
    fn should_scan_unchanged(&self, _stored: &StoredScanState) -> bool {
        false
    }
    /// Update file compatibility from per-record checks, committing it with events and cursors.
    fn scan_format(&self, _outcome: &ScanOutcome) -> Option<DetectOutcome> {
        None
    }
    /// Structured capability declaration.
    fn capability(&self) -> CapabilityTable;

    /// Complete prior hashes for explicitly verified parser corrections only.
    /// Require the same source revision; unrelated field changes still conflict.
    fn prior_aggregate_hashes(
        &self,
        _input: &crate::aggregates::SourceAggregateInput,
    ) -> Vec<String> {
        Vec::new()
    }

    /// A verified archive may replace file scanning for this root.
    /// Keep errors visible without silently adding counts from a second data format.
    fn scan_archive(
        &self,
        _storage: &Storage,
        _root: &DiscoveredRoot,
        _config: &RunConfig,
    ) -> Result<Option<BatchOutcome>, CoreError> {
        Ok(None)
    }
}

/// Normalize separators and remove the Windows verbatim prefix for local identity.
pub fn normalize_path(path: &Path) -> String {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").map(str::to_string).unwrap_or(s)
}

/// Enumerate bounded depth/file/directory counts without following symlinks.
pub fn enumerate_files_bounded(
    root: &Path,
    max_depth: usize,
    accept: &dyn Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut dirs_visited = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if super::run_policy::check().is_err() {
            break;
        }
        if dirs_visited >= DISCOVER_MAX_DIRS || files.len() >= DISCOVER_MAX_FILES {
            break;
        }
        dirs_visited += 1;
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if super::run_policy::check().is_err() {
                break;
            }
            if files.len() >= DISCOVER_MAX_FILES {
                break;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if depth < max_depth {
                    stack.push((path, depth + 1));
                }
            } else if meta.is_file() && accept(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Rotate bounded file windows using persisted visits; unvisited files go first.
/// Equal visits retain the adapter's discovery order. Only committed file rows
/// count as visits, so a rolled-back read keeps its place for the next run.
fn files_in_scan_order(
    storage: &Storage,
    instance_id: &str,
    files: &[PathBuf],
) -> Result<Vec<PathBuf>, CoreError> {
    let mut visits: std::collections::BTreeMap<String, Option<i64>> = files
        .iter()
        .map(|path| (normalize_path(path), None))
        .collect();
    let mut stmt = storage
        .conn()
        .prepare("SELECT file_id,last_seen_ms FROM source_files WHERE instance_id=?1")?;
    let rows = stmt.query_map([instance_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        super::run_policy::check()?;
        let (file_id, seen_ms) = row?;
        if let Some(visit) = visits.get_mut(&file_id) {
            *visit = Some(seen_ms);
        }
    }
    let mut ordered = files.to_vec();
    ordered.sort_by_cached_key(|path| visits.get(&normalize_path(path)).copied().flatten());
    Ok(ordered)
}

/// source_instances registration input; preserve user settings such as enabled during conflict updates.
/// Verified local collection supplies origin_host_id; None denotes legacy_unknown ownership.
/// Preserve sources owned by another host; verified local collection may claim legacy_unknown sources
/// when file locality and ownership mapping are verified (data-contract.md#provenance).
#[derive(Debug, Clone)]
pub struct SourceInstanceInput {
    pub instance_id: String,
    pub agent: String,
    pub host_application: Option<String>,
    pub locality_basis: crate::domain::LocalityBasis,
    pub attribution_status: crate::domain::AttributionStatus,
    pub exclusion_reason: Option<String>,
    pub format: String,
    pub location_hint: Option<String>,
    pub parser_version: String,
    pub capabilities: serde_json::Value,
    pub health: String,
    /// Source host ownership (M1a); None uses the legacy_unknown namespace.
    pub origin_host_id: Option<String>,
}

/// Historical namespace for databases before v4 host ownership.
pub const LEGACY_UNKNOWN_HOST: &str = "legacy_unknown";

pub fn upsert_source_instance(
    storage: &Storage,
    input: &SourceInstanceInput,
    now_ms: i64,
) -> Result<(), CoreError> {
    let host_id = input
        .origin_host_id
        .clone()
        .unwrap_or_else(|| LEGACY_UNKNOWN_HOST.to_string());
    let tx = storage.conn().unchecked_transaction()?;
    let previous: Option<(Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT parser_version, capabilities FROM source_instances WHERE instance_id=?1",
            [&input.instance_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if previous.is_some_and(|(parser, capabilities)| {
        let old: serde_json::Value = capabilities
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        parser.as_deref() != Some(&input.parser_version)
            || old["supported_versions"] != input.capabilities["supported_versions"]
    }) {
        tx.execute(
            "DELETE FROM ingestion_checkpoints WHERE instance_id=?1",
            [&input.instance_id],
        )?;
        tx.execute(
            "UPDATE source_files SET status='new', format_status=NULL WHERE instance_id=?1",
            [&input.instance_id],
        )?;
    }
    tx.execute(
        "INSERT INTO source_instances (
           instance_id, agent, host_application, locality_basis, attribution_status,
           exclusion_reason, enabled, format, location_hint, parser_version, capabilities,
           health, origin_host_id, created_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
         ON CONFLICT(instance_id) DO UPDATE SET
           agent = excluded.agent,
           host_application = excluded.host_application,
           locality_basis = excluded.locality_basis,
           attribution_status = excluded.attribution_status,
           format = excluded.format,
           location_hint = excluded.location_hint,
           parser_version = excluded.parser_version,
           capabilities = excluded.capabilities,
           health = excluded.health,
           origin_host_id = CASE
             WHEN source_instances.origin_host_id = 'legacy_unknown'
                  AND excluded.origin_host_id != 'legacy_unknown'
             THEN excluded.origin_host_id
             ELSE source_instances.origin_host_id
           END,
           updated_at_ms = excluded.updated_at_ms",
        params![
            input.instance_id,
            input.agent,
            input.host_application,
            input.locality_basis.as_str(),
            input.attribution_status.as_str(),
            input.exclusion_reason,
            input.format,
            input.location_hint,
            input.parser_version,
            serde_json::to_string(&input.capabilities)?,
            input.health,
            host_id,
            now_ms
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// source_files row used for registration, rename and replacement detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileRow {
    pub file_id: String,
    pub file_identity: String,
    pub generation: i64,
    pub len: u64,
    pub mtime_ms: i64,
    pub created_ms: Option<i64>,
    pub head_hash: u64,
    pub head_len: u64,
    pub tail_hash: u64,
    pub status: String,
    /// Detection JSON: native version, selected format/parser, selection basis and compatibility (v3 column).
    pub format_status: Option<String>,
}

pub fn load_source_file(
    storage: &Storage,
    instance_id: &str,
    file_id: &str,
) -> Result<Option<SourceFileRow>, CoreError> {
    let row = storage
        .conn()
        .query_row(
            "SELECT file_id, file_identity, generation, byte_size, mtime_ms, status, 0, content_hash, format_status
             FROM source_files WHERE instance_id = ?1 AND file_id = ?2",
            params![instance_id, file_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, Option<String>>(8)?,
                ))
            },
        )
        .optional()?;
    match row {
        Some((
            file_id,
            file_identity,
            generation,
            byte_size,
            mtime_ms,
            status,
            composite,
            format_status,
        )) => {
            let mut parts = composite.split(':');
            let parse = |p: Option<&str>| p.and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let head_hash = parse(parts.next());
            let head_len = parse(parts.next());
            let tail_hash = parse(parts.next());
            let created_ms = parts
                .next()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|&v| v >= 0);
            Ok(Some(SourceFileRow {
                file_id,
                file_identity,
                generation,
                len: byte_size.unwrap_or(0) as u64,
                mtime_ms: mtime_ms.unwrap_or(0),
                created_ms,
                head_hash,
                head_len,
                tail_hash,
                status,
                format_status,
            }))
        }
        None => Ok(None),
    }
}

/// Find by file identity: a supported rename preserves the content stream while changing its path.
pub fn find_source_file_by_identity(
    storage: &Storage,
    instance_id: &str,
    file_identity: &str,
) -> Result<Option<SourceFileRow>, CoreError> {
    let found: Option<String> = storage
        .conn()
        .query_row(
            "SELECT file_id FROM source_files WHERE instance_id = ?1 AND file_identity = ?2",
            params![instance_id, file_identity],
            |r| r.get(0),
        )
        .optional()?;
    match found {
        Some(file_id) => load_source_file(storage, instance_id, &file_id),
        None => Ok(None),
    }
}

pub fn upsert_source_file(
    storage: &Storage,
    instance_id: &str,
    row: &SourceFileRow,
    now_ms: i64,
) -> Result<(), CoreError> {
    let composite = format!(
        "{}:{}:{}:{}",
        row.head_hash,
        row.head_len,
        row.tail_hash,
        row.created_ms.unwrap_or(-1)
    );
    storage.conn().execute(
        "INSERT INTO source_files (
           file_id, instance_id, file_identity, generation, byte_size, mtime_ms,
           content_hash, status, format_status, first_seen_ms, last_seen_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(instance_id, file_identity) DO UPDATE SET
           file_id = excluded.file_id,
           generation = excluded.generation,
           byte_size = excluded.byte_size,
           mtime_ms = excluded.mtime_ms,
           content_hash = excluded.content_hash,
           status = excluded.status,
           format_status = excluded.format_status,
           last_seen_ms = excluded.last_seen_ms",
        params![
            row.file_id,
            instance_id,
            row.file_identity,
            row.generation,
            row.len as i64,
            row.mtime_ms,
            composite,
            row.status,
            row.format_status,
            now_ms
        ],
    )?;
    Ok(())
}

/// Load cursor and parse context from the checkpoint.
pub fn load_scan_state(
    storage: &Storage,
    instance_id: &str,
    scope_key: &str,
) -> Result<StoredScanState, CoreError> {
    let row: Option<(Option<String>, Option<String>)> = storage
        .conn()
        .query_row(
            "SELECT cursor_value, parse_context FROM ingestion_checkpoints
             WHERE instance_id = ?1 AND scope_key = ?2",
            params![instance_id, scope_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((cursor, context)) => Ok(StoredScanState {
            cursor: cursor.and_then(|v| serde_json::from_str(&v).ok()),
            parse_context: context.and_then(|v| serde_json::from_str(&v).ok()),
        }),
        None => Ok(StoredScanState::default()),
    }
}

/// Build StoredFileState from source_files for file-continuity checks.
pub fn stored_file_state(row: &SourceFileRow, cursor_offset: u64) -> StoredFileState {
    StoredFileState {
        generation: row.generation,
        len: row.len,
        created_ms: row.created_ms,
        head_hash: row.head_hash,
        head_len: row.head_len,
        tail_hash: row.tail_hash,
        cursor_offset,
    }
}

/// Configuration for one collection run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub timezone: String,
    pub now_ms: i64,
    pub limits: ScanLimits,
    pub trigger: TriggerKind,
    /// Run/batch ID prefix, with the instance sequence appended.
    pub run_id_prefix: String,
    /// Local source-host ID (M1a); None denotes legacy_unknown ownership.
    /// The application obtains it through Storage::ensure_local_host.
    pub origin_host_id: Option<String>,
}

/// Per-file run report.
#[derive(Debug, Clone)]
pub struct FileReport {
    pub file_id: String,
    pub status: String,
    pub detail: Option<String>,
    pub lines_read: u64,
    pub records_seen: u64,
    pub events: u64,
    pub diagnostics: u64,
}

/// Per-instance run report.
#[derive(Debug, Clone)]
pub struct SourceRunReport {
    pub instance_id: String,
    pub run_id: Option<String>,
    pub start: Option<RunStart>,
    pub outcome: Option<BatchOutcome>,
    pub files: Vec<FileReport>,
    pub reconciliations: Vec<Reconciliation>,
    pub finish: RunStatus,
    pub error: Option<String>,
}

/// Discover and detect/scan each file, then commit the instance in one transaction (V12).
/// Return instance reports; isolate file failures and mark an unsuccessful instance commit failed.
pub fn run_adapter_scan(
    storage: &Storage,
    adapter: &dyn SourceAdapter,
    ctx: &DiscoverContext,
    config: &RunConfig,
) -> Result<Vec<SourceRunReport>, CoreError> {
    run_adapter_scan_filtered(storage, adapter, ctx, config, &InstanceFilter::default())
}

/// Instance filters support source schedules: custom-scheduled sources use their own triggers,
/// global refresh excludes them, and due-source refresh includes only due instances.
#[derive(Debug, Clone, Default)]
pub struct InstanceFilter {
    /// Include these instances only; None imposes no inclusion filter.
    pub include: Option<BTreeSet<String>>,
    /// Skip these instances; None imposes no exclusion filter.
    pub exclude: Option<BTreeSet<String>>,
}

impl InstanceFilter {
    fn allows(&self, instance_id: &str) -> bool {
        if let Some(include) = &self.include {
            if !include.contains(instance_id) {
                return false;
            }
        }
        if let Some(exclude) = &self.exclude {
            if exclude.contains(instance_id) {
                return false;
            }
        }
        true
    }
}

/// Filtered run_adapter_scan for source scheduling.
/// Check source_instances.enabled before reading from any trigger:
/// Interval, Startup, Manual or FixedTime. Read only enabled local sources,
/// preserving set_source_enabled behavior after an instance is disabled.
/// If disabled-instance loading fails, stop this adapter before reading sources.
pub fn run_adapter_scan_filtered(
    storage: &Storage,
    adapter: &dyn SourceAdapter,
    ctx: &DiscoverContext,
    config: &RunConfig,
    filter: &InstanceFilter,
) -> Result<Vec<SourceRunReport>, CoreError> {
    run_adapter_scan_filtered_controlled(
        storage,
        adapter,
        ctx,
        config,
        filter,
        &RunControl {
            deadline: None,
            allowed: &|| true,
        },
    )
}

pub struct RunControl<'a> {
    pub deadline: Option<std::time::Instant>,
    pub allowed: &'a (dyn Fn() -> bool + Sync),
}
impl RunControl<'_> {
    fn interrupted(&self) -> bool {
        !(self.allowed)()
            || self
                .deadline
                .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }
    fn check(&self) -> Result<(), CoreError> {
        if self.interrupted() {
            Err(CoreError::Interrupted(
                "automatic_scan_paused_or_round_deadline",
            ))
        } else {
            Ok(())
        }
    }
    fn wait(&self, delay: std::time::Duration) -> Result<(), CoreError> {
        let until = std::time::Instant::now() + delay;
        loop {
            self.check()?;
            let remaining = until.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Ok(());
            }
            std::thread::sleep(remaining.min(std::time::Duration::from_millis(100)));
        }
    }
}

/// Keep source parsing outside the writer lock; metadata and commits use it.
pub trait StorageAccess {
    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&Storage) -> Result<T, CoreError>,
    ) -> Result<T, CoreError>;
}

pub struct ParallelScanRequest<'a> {
    pub adapter: &'a dyn SourceAdapter,
    pub context: DiscoverContext,
    pub config: RunConfig,
    pub filter: InstanceFilter,
}

struct RootAdapter<'a> {
    adapter: &'a dyn SourceAdapter,
    root: DiscoveredRoot,
}
impl SourceAdapter for RootAdapter<'_> {
    fn adapter_id(&self) -> &'static str {
        self.adapter.adapter_id()
    }
    fn agent(&self) -> &'static str {
        self.adapter.agent()
    }
    fn discover(&self, _: &DiscoverContext) -> Vec<DiscoveredRoot> {
        vec![self.root.clone()]
    }
    fn rotate_file_windows(&self) -> bool {
        self.adapter.rotate_file_windows()
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        self.adapter.instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        self.adapter.detect(path)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError> {
        self.adapter.scan(target, stored, limits, now_ms)
    }
    fn should_scan_unchanged(&self, stored: &StoredScanState) -> bool {
        self.adapter.should_scan_unchanged(stored)
    }
    fn scan_format(&self, outcome: &ScanOutcome) -> Option<DetectOutcome> {
        self.adapter.scan_format(outcome)
    }
    fn capability(&self) -> CapabilityTable {
        self.adapter.capability()
    }
    fn prior_aggregate_hashes(
        &self,
        input: &crate::aggregates::SourceAggregateInput,
    ) -> Vec<String> {
        self.adapter.prior_aggregate_hashes(input)
    }
    fn scan_archive(
        &self,
        storage: &Storage,
        root: &DiscoveredRoot,
        config: &RunConfig,
    ) -> Result<Option<BatchOutcome>, CoreError> {
        self.adapter.scan_archive(storage, root, config)
    }
}

/// Two source-instance slots, one existing writer; results retain request/root order.
pub fn run_adapter_scans_parallel<S: StorageAccess + Sync>(
    access: &S,
    requests: &[ParallelScanRequest<'_>],
    deadline: Option<std::time::Instant>,
    allowed: super::run_policy::Allowed,
    instance_allowed: Option<super::run_policy::InstanceAllowed>,
) -> Vec<Result<Vec<SourceRunReport>, CoreError>> {
    let _discovery = super::run_policy::enter(deadline, Some(allowed.clone()));
    let mut work = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        if super::run_policy::check().is_err() {
            break;
        }
        let mut seen = BTreeSet::new();
        for (root_index, root) in request
            .adapter
            .discover(&request.context)
            .into_iter()
            .enumerate()
        {
            let path = normalize_path(&root.root);
            let key = if cfg!(windows) {
                path.to_lowercase()
            } else {
                path
            };
            if seen.insert(key) {
                work.push((
                    index,
                    root_index,
                    RootAdapter {
                        adapter: request.adapter,
                        root,
                    },
                ));
            }
        }
    }
    drop(_discovery);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut completed = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..work.len().min(2))
            .map(|_| {
                let allowed = allowed.clone();
                let instance_allowed = instance_allowed.clone();
                let work = &work;
                let next = &next;
                std::thread::Builder::new()
                    .name("usage-source".into())
                    .spawn_scoped(scope, move || {
                        let mut results = Vec::new();
                        loop {
                            let position = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some((index, root_index, adapter)) = work.get(position) else {
                                break;
                            };
                            let request = &requests[*index];
                            let _scope = super::run_policy::enter(deadline, Some(allowed.clone()));
                            super::run_policy::enter_instances(instance_allowed.clone());
                            let config = RunConfig {
                                run_id_prefix: format!(
                                    "{}-root{root_index}",
                                    request.config.run_id_prefix
                                ),
                                ..request.config.clone()
                            };
                            let result = run_adapter_scan_filtered_access(
                                access,
                                adapter,
                                &request.context,
                                &config,
                                &request.filter,
                                &RunControl {
                                    deadline,
                                    allowed: allowed.as_ref(),
                                },
                            );
                            results.push((*index, *root_index, result));
                        }
                        results
                    })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| match handle {
                Ok(handle) => handle
                    .join()
                    .map_err(|_| CoreError::JobState("source_worker_panicked".into())),
                Err(error) => Err(error.into()),
            })
            .collect::<Vec<_>>()
    });
    if completed.iter().any(Result::is_err) {
        // A worker panic may leave a running job; normal Storage recovery handles it.
        return requests
            .iter()
            .map(|_| Err(CoreError::JobState("source_worker_failed".into())))
            .collect();
    }
    let mut completed: Vec<_> = completed.drain(..).flat_map(|r| r.unwrap()).collect();
    completed.sort_by_key(|(index, root, _)| (*index, *root));
    let mut results: Vec<Result<Vec<SourceRunReport>, CoreError>> =
        requests.iter().map(|_| Ok(Vec::new())).collect();
    for (index, _, result) in completed {
        match result {
            Ok(reports) => {
                if let Ok(previous) = &mut results[index] {
                    previous.extend(reports);
                }
            }
            Err(error) => results[index] = Err(error),
        }
    }
    results
}
impl StorageAccess for Storage {
    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&Storage) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        operation(self)
    }
}
impl StorageAccess for std::sync::Mutex<Storage> {
    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&Storage) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let storage = self
            .lock()
            .map_err(|_| CoreError::JobState("writer_lock_poisoned".into()))?;
        operation(&storage)
    }
}

pub fn run_adapter_scan_filtered_controlled(
    storage: &Storage,
    adapter: &dyn SourceAdapter,
    ctx: &DiscoverContext,
    config: &RunConfig,
    filter: &InstanceFilter,
    control: &RunControl<'_>,
) -> Result<Vec<SourceRunReport>, CoreError> {
    run_adapter_scan_filtered_access(storage, adapter, ctx, config, filter, control)
}

/// Metadata and commits share the existing writer; source parsing holds no writer lock.
pub fn run_adapter_scan_filtered_access<S: StorageAccess>(
    access: &S,
    adapter: &dyn SourceAdapter,
    ctx: &DiscoverContext,
    config: &RunConfig,
    filter: &InstanceFilter,
    control: &RunControl<'_>,
) -> Result<Vec<SourceRunReport>, CoreError> {
    let disabled: BTreeSet<String> = access.with_storage(|storage| {
        let mut stmt = storage
            .conn()
            .prepare("SELECT instance_id FROM source_instances WHERE enabled=0")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut set = BTreeSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    })?;
    let mut reports = Vec::new();
    let roots = adapter.discover(ctx);
    // Deduplicate environment/default/manual roots; normalize Windows case aliases first.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let roots: Vec<DiscoveredRoot> = roots
        .into_iter()
        .filter(|r| {
            let path = normalize_path(&r.root);
            seen.insert(if cfg!(windows) {
                path.to_lowercase()
            } else {
                path
            })
        })
        .collect();
    let capability = adapter.capability();
    for (index, discovered) in roots.iter().enumerate() {
        let mut root = discovered.clone();
        let instance_id = adapter.instance_id(&root);
        if disabled.contains(&instance_id) || !filter.allows(&instance_id) {
            continue;
        }
        if root.basis == RootBasis::Manual {
            let mut files = Vec::new();
            for path in &root.files {
                if access.with_storage(|storage| {
                    super::routing::accepts_manual_file(storage, &instance_id, path)
                })? {
                    files.push(path.clone());
                }
            }
            // Archive adapters use an empty file list and perform their own detection.
            if !root.files.is_empty() && files.is_empty() {
                continue;
            }
            root.files = files;
        }
        let root = &root;
        let mut report = SourceRunReport {
            instance_id: instance_id.clone(),
            run_id: None,
            start: None,
            outcome: None,
            files: Vec::new(),
            reconciliations: Vec::new(),
            finish: RunStatus::Succeeded,
            error: None,
        };
        if control.interrupted() {
            report.finish = RunStatus::Interrupted;
            report.error = Some("automatic_scan_paused_or_round_deadline; source not read".into());
            reports.push(report);
            continue;
        }
        access.with_storage(|storage| {
            upsert_source_instance(
                storage,
                &SourceInstanceInput {
                    instance_id: instance_id.clone(),
                    agent: adapter.agent().to_string(),
                    host_application: None,
                    locality_basis: crate::domain::LocalityBasis::LocalFilesystem,
                    attribution_status: crate::domain::AttributionStatus::Verified,
                    exclusion_reason: None,
                    format: adapter.adapter_id().to_string(),
                    location_hint: Some(normalize_path(&root.root)),
                    parser_version: capability.maintenance["parser_version"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string(),
                    capabilities: serde_json::to_value(&capability)?,
                    health: storage
                        .conn()
                        .query_row(
                            "SELECT health FROM source_instances WHERE instance_id=?1",
                            [&instance_id],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()?
                        .unwrap_or_else(|| "ok".into()),
                    origin_host_id: config.origin_host_id.clone(),
                },
                config.now_ms,
            )
        })?;
        let run_id = format!("{}-{index}", config.run_id_prefix);
        match access.with_storage(|storage| {
            jobs::start_run(
                storage,
                &run_id,
                &instance_id,
                config.trigger,
                config.now_ms,
            )
        }) {
            Ok(start) => {
                report.run_id = Some(match &start {
                    RunStart::Started(id) | RunStart::Merged(id) => id.clone(),
                });
                report.start = Some(start);
            }
            Err(e) => {
                report.finish = RunStatus::Failed;
                report.error = Some(e.to_string());
                reports.push(report);
                continue;
            }
        }
        let active_run = report.run_id.clone().unwrap_or(run_id);
        if matches!(report.start, Some(RunStart::Merged(_))) {
            // A merged request performed no scan here; do not report it as completed.
            report.finish = RunStatus::Running;
            reports.push(report);
            continue;
        }
        let source_started = std::time::Instant::now();
        let _source_scope = super::run_policy::enter_source(
            &instance_id,
            config
                .limits
                .jsonl
                .time_budget
                .map(|budget| source_started + budget),
        );
        if config
            .limits
            .jsonl
            .time_budget
            .is_some_and(|budget| budget.is_zero())
        {
            report.finish = RunStatus::Interrupted;
            report.error = Some("source_time_budget_exhausted; source not read".into());
            access.with_storage(|storage| {
                jobs::finish_run(
                    storage,
                    &active_run,
                    report.finish,
                    RunStats::default(),
                    report.error.as_deref(),
                    config.now_ms,
                )
            })?;
            reports.push(report);
            continue;
        }
        let mut files = root.files.clone();
        let archive = access
            .with_storage(|storage| {
                super::run_policy::check()?;
                let enabled = storage.conn().query_row(
                    "SELECT enabled FROM source_instances WHERE instance_id=?1",
                    [&instance_id],
                    |r| r.get::<_, bool>(0),
                )?;
                if !enabled {
                    return Err(CoreError::Interrupted("source_disabled"));
                }
                let _sql = super::run_policy::SqliteScope::new(storage.conn())?;
                let archive = adapter.scan_archive(storage, root, config)?;
                if archive.is_none() && adapter.rotate_file_windows() {
                    // Preparation failures and cancellation finish the run just
                    // like archive failures, without leaving a running job behind.
                    files = files_in_scan_order(storage, &instance_id, &root.files)?;
                }
                Ok(archive)
            })
            .map_err(|error| super::run_policy::check().err().unwrap_or(error));
        match archive {
            Ok(Some(outcome)) => {
                if control.interrupted()
                    || config
                        .limits
                        .jsonl
                        .time_budget
                        .is_some_and(|budget| source_started.elapsed() >= budget)
                {
                    report.finish = RunStatus::Interrupted;
                    report.error = Some(
                        "source_or_round_time_budget_exhausted; confirmed archive retained".into(),
                    );
                }
                let stats = RunStats {
                    added: outcome.added,
                    updated: outcome.updated,
                    unchanged: outcome.unchanged,
                    skipped: outcome.skipped,
                    errors: outcome.errors,
                };
                report.outcome = Some(outcome);
                access.with_storage(|storage| {
                    jobs::finish_run(
                        storage,
                        &active_run,
                        report.finish,
                        stats,
                        report.error.as_deref(),
                        config.now_ms,
                    )
                })?;
                reports.push(report);
                continue;
            }
            Err(e) => {
                report.finish = if matches!(e, CoreError::Interrupted(_)) {
                    RunStatus::Interrupted
                } else {
                    RunStatus::Failed
                };
                report.error = Some(e.to_string());
                access.with_storage(|storage| {
                    jobs::finish_run(
                        storage,
                        &active_run,
                        report.finish,
                        RunStats::default(),
                        report.error.as_deref(),
                        config.now_ms,
                    )
                })?;
                reports.push(report);
                continue;
            }
            Ok(None) => {}
        }
        let mut batch = IngestBatch {
            batch_id: format!("{}-batch", active_run),
            instance_id: instance_id.clone(),
            timezone: config.timezone.clone(),
            now_ms: config.now_ms,
            events: Vec::new(),
            checkpoints: Vec::new(),
            diagnostics: Vec::new(),
            run_id: Some(active_run.clone()),
            retention_cutoff_ms: None,
        };
        let mut aggregates: Vec<crate::aggregates::SourceAggregateInput> = Vec::new();
        let mut file_updates = Vec::new();
        let mut any_scanned = false;
        for path in &files {
            let enabled = access.with_storage(|storage| {
                Ok(storage.conn().query_row(
                    "SELECT enabled FROM source_instances WHERE instance_id=?1",
                    [&instance_id],
                    |r| r.get::<_, bool>(0),
                )?)
            })?;
            if !enabled {
                report.finish = RunStatus::Interrupted;
                report.error = Some("source_disabled; remaining files retained".into());
                break;
            }
            if control.interrupted() {
                report.finish = RunStatus::Interrupted;
                report.error = Some(
                    "automatic_scan_paused_or_round_deadline; remaining files retained".into(),
                );
                break;
            }
            let mut file_config = config.clone();
            if let Some(budget) = config.limits.jsonl.time_budget {
                let remaining = budget.saturating_sub(source_started.elapsed());
                if remaining.is_zero() {
                    report.finish = RunStatus::Interrupted;
                    report.error = Some(
                        "source_time_budget_exhausted; remaining files retained for the next scan"
                            .into(),
                    );
                    break;
                }
                file_config.limits.jsonl.time_budget = Some(remaining);
            }
            if let Some(deadline) = control.deadline {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                file_config.limits.jsonl.time_budget = Some(
                    file_config
                        .limits
                        .jsonl
                        .time_budget
                        .map_or(remaining, |budget| budget.min(remaining)),
                );
            }
            let file_report = scan_one_file(
                access,
                adapter,
                path,
                &instance_id,
                &file_config,
                &mut batch,
                &mut aggregates,
                &mut report.reconciliations,
                &mut file_updates,
                control,
            );
            match file_report {
                Ok((fr, scanned)) => {
                    any_scanned |= scanned;
                    let incomplete = fr.status == ScanStatus::BudgetExhausted.as_str();
                    report.files.push(fr);
                    if incomplete {
                        report.finish = RunStatus::Interrupted;
                        report.error = Some(
                            "source_window_budget_exhausted; complete lines retained for next scan"
                                .into(),
                        );
                        break;
                    }
                    if config
                        .limits
                        .jsonl
                        .time_budget
                        .is_some_and(|budget| source_started.elapsed() >= budget)
                    {
                        report.finish = RunStatus::Interrupted;
                        report.error = Some(
                            "source_time_budget_exhausted; confirmed events and cursor retained"
                                .into(),
                        );
                        break;
                    }
                }
                Err(e @ CoreError::Interrupted(_)) => {
                    report.finish = RunStatus::Interrupted;
                    report.error = Some(e.to_string());
                    break;
                }
                Err(e) => {
                    report.finish = RunStatus::Failed;
                    report.error = Some(e.to_string());
                    report.files.push(FileReport {
                        file_id: normalize_path(path),
                        status: "error".to_string(),
                        detail: Some(e.to_string()),
                        lines_read: 0,
                        records_seen: 0,
                        events: 0,
                        diagnostics: 0,
                    });
                }
            }
        }
        if report.finish == RunStatus::Succeeded && control.interrupted() {
            report.finish = RunStatus::Interrupted;
            report.error =
                Some("automatic_scan_paused_or_round_deadline; confirmed events retained".into());
        }
        // Skip commits when no file changed; unchanged scans do not advance the data revision.
        if any_scanned || !batch.events.is_empty() || !batch.checkpoints.is_empty() {
            let committed = access.with_storage(|storage| {
                super::run_policy::check()?;
                if !storage.conn().query_row(
                    "SELECT enabled FROM source_instances WHERE instance_id=?1",
                    [&instance_id],
                    |r| r.get::<_, bool>(0),
                )? {
                    return Err(CoreError::Interrupted("source_disabled"));
                }
                let _sql = super::run_policy::SqliteScope::new(storage.conn())?;
                let tx = storage.conn().unchecked_transaction()?;
                for row in &file_updates {
                    super::run_policy::check()?;
                    upsert_source_file(storage, &instance_id, row, config.now_ms)?;
                }
                let mut outcome = ingest::commit_batch_tx(storage, &tx, &batch, None, false)?;
                // Commit native aggregates with events and cursors; any failure permits a complete replay.
                for aggregate in &aggregates {
                    super::run_policy::check()?;
                    crate::aggregates::upsert_source_aggregate_with_prior_hashes_tx(
                        &tx,
                        aggregate,
                        config.now_ms,
                        &adapter.prior_aggregate_hashes(aggregate),
                    )?;
                }
                outcome.data_revision = storage.data_revision()?;
                super::run_policy::check()?;
                tx.commit()?;
                Ok::<_, CoreError>(outcome)
            });
            match committed {
                Ok(outcome) => report.outcome = Some(outcome),
                Err(e) => {
                    let e = super::run_policy::check().err().unwrap_or(e);
                    report.finish = if matches!(e, CoreError::Interrupted(_)) {
                        RunStatus::Interrupted
                    } else {
                        RunStatus::Failed
                    };
                    report.error = Some(e.to_string());
                    let _ = access.with_storage(|storage| {
                        jobs::finish_run(
                            storage,
                            &active_run,
                            report.finish,
                            RunStats::default(),
                            Some(&e.to_string()),
                            config.now_ms,
                        )
                    });
                    reports.push(report);
                    continue;
                }
            }
        }
        let stats = report
            .outcome
            .as_ref()
            .map(|o| RunStats {
                added: o.added,
                updated: o.updated,
                unchanged: o.unchanged,
                skipped: o.skipped,
                errors: o.errors,
            })
            .unwrap_or_default();
        access.with_storage(|storage| {
            jobs::finish_run(
                storage,
                &active_run,
                report.finish,
                stats,
                report.error.as_deref(),
                config.now_ms,
            )
        })?;
        reports.push(report);
    }
    Ok(reports)
}

enum FilePreparation {
    Skipped(FileReport, bool),
    Read(Box<PreparedFile>),
}
struct PreparedFile {
    target: ScanTarget,
    stored: StoredScanState,
    row: SourceFileRow,
    detect_basis: Option<crate::domain::VersionBasis>,
}

/// Register one file, check continuity, detect when needed, scan and collect batch results.
/// Return the file report and whether an actual read occurred.
#[allow(clippy::too_many_arguments)]
fn scan_one_file<S: StorageAccess>(
    access: &S,
    adapter: &dyn SourceAdapter,
    path: &Path,
    instance_id: &str,
    config: &RunConfig,
    batch: &mut IngestBatch,
    aggregates: &mut Vec<crate::aggregates::SourceAggregateInput>,
    reconciliations: &mut Vec<Reconciliation>,
    file_updates: &mut Vec<SourceFileRow>,
    control: &RunControl<'_>,
) -> Result<(FileReport, bool), CoreError> {
    let file_started = std::time::Instant::now();
    let _file_scope = super::run_policy::enter(
        config
            .limits
            .jsonl
            .time_budget
            .map(|budget| file_started + budget),
        None,
    );
    let prepared_result = access.with_storage(|storage| {
    let file_id = normalize_path(path);
    if !super::routing::claim_file(storage, adapter, instance_id, path)? {
        return Ok(FilePreparation::Skipped(
            FileReport {
                file_id,
                status: "owned_elsewhere".into(),
                detail: Some("file already belongs to another source; not counted twice".into()),
                lines_read: 0,
                records_seen: 0,
                events: 0,
                diagnostics: 0,
            },
            false,
        ));
    }
    let probe = super::jsonl::probe_file(path)?;
    let identity = file_identity_of(&probe);
    // After a path miss, check file identity; renaming the same stream does not create a new file.
    let mut row = match load_source_file(storage, instance_id, &file_id)? {
        Some(row) => row,
        None => match find_source_file_by_identity(storage, instance_id, &identity)? {
            Some(mut row) => {
                row.file_id = file_id.clone();
                row
            }
            None => SourceFileRow {
                file_id: file_id.clone(),
                file_identity: identity.clone(),
                generation: 0,
                len: 0,
                mtime_ms: 0,
                created_ms: probe.created_ms,
                head_hash: probe.head_hash,
                head_len: probe.head_len,
                tail_hash: probe.tail_hash,
                status: "new".to_string(),
                format_status: None,
            },
        },
    };
    let stored = load_scan_state(storage, instance_id, &row.file_identity)?;
    let cursor_offset = stored
        .cursor
        .as_ref()
        .and_then(|c| c.get("offset"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let decision = super::jsonl::decide_generation(&stored_file_state(&row, cursor_offset), &probe);
    let rescan = matches!(decision, super::jsonl::GenerationDecision::Rescan(_));
    if rescan {
        row.generation += 1;
    }
    // Skip unchanged files whose bytes are fully consumed and continuity is intact.
    if !rescan
        && probe.len == cursor_offset
        && stored.cursor.is_some()
        && !adapter.should_scan_unchanged(&stored)
    {
        row.len = probe.len;
        row.mtime_ms = probe.mtime_ms;
        upsert_source_file(storage, instance_id, &row, config.now_ms)?;
        return Ok(FilePreparation::Skipped(
            FileReport {
                file_id,
                status: "unchanged".to_string(),
                detail: None,
                lines_read: 0,
                records_seen: 0,
                events: 0,
                diagnostics: 0,
            },
            false,
        ));
    }
    // Detect again on first scan/rescan; restrict verified incompatible versions and unknown formats (V17).
    // Unknown/missing versions use the latest registered Agent reader and retain compatibility metadata.
    let mut detect_basis: Option<crate::domain::VersionBasis> = None;
    if stored.cursor.is_none() || rescan {
            let detected = adapter.detect(path);
            super::run_policy::check()?;
            match detected? {
            DetectOutcome::Supported {
                format,
                format_version,
                basis,
            } => {
                row.format_status = Some(format_status_json(
                    &format,
                    format_version.as_deref(),
                    basis,
                ));
                if basis == crate::domain::VersionBasis::LatestFallback {
                    batch.diagnostics.push(DiagnosticInput {
                        event_id: None,
                        code: "latest_fallback".to_string(),
                        field: Some("version".to_string()),
                        position: Some(file_id.clone()),
                        message: format!(
                            "using latest built-in parser; version compatibility unverified (found: {})",
                            format_version.as_deref().unwrap_or("missing")
                        ),
                    });
                }
                detect_basis = Some(basis);
            }
            DetectOutcome::Pending => {
                row.status = "pending".to_string();
                upsert_source_file(storage, instance_id, &row, config.now_ms)?;
                return Ok(FilePreparation::Skipped(
                    FileReport {
                        file_id,
                        status: "pending".to_string(),
                        detail: None,
                        lines_read: 0,
                        records_seen: 0,
                        events: 0,
                        diagnostics: 0,
                    },
                    false,
                ));
            }
            DetectOutcome::UnsupportedVersion { found, reason, .. } => {
                row.status = "unsupported".to_string();
                upsert_source_file(storage, instance_id, &row, config.now_ms)?;
                batch.diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "unsupported_version".to_string(),
                    field: Some("version".to_string()),
                    position: Some(file_id.clone()),
                    message: format!(
                        "evidenced incompatible version {:?}; fail closed: {reason}",
                        found.clone().unwrap_or_else(|| "missing".to_string())
                    ),
                });
                return Ok(FilePreparation::Skipped(
                    FileReport {
                        file_id,
                        status: "unsupported_version".to_string(),
                        detail: Some(found.unwrap_or_else(|| "missing".to_string())),
                        lines_read: 0,
                        records_seen: 0,
                        events: 0,
                        diagnostics: 1,
                    },
                    true,
                ));
            }
            DetectOutcome::UnknownFormat { reason } => {
                row.status = "unsupported".to_string();
                upsert_source_file(storage, instance_id, &row, config.now_ms)?;
                batch.diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "unknown_format".to_string(),
                    field: None,
                    position: Some(file_id.clone()),
                    message: format!("unknown format; fail closed: {reason}"),
                });
                return Ok(FilePreparation::Skipped(
                    FileReport {
                        file_id,
                        status: "unknown_format".to_string(),
                        detail: Some(reason),
                        lines_read: 0,
                        records_seen: 0,
                        events: 0,
                        diagnostics: 1,
                    },
                    true,
                ));
            }
        }
    }
    let target = ScanTarget {
        instance_id: instance_id.to_string(),
        path: path.to_path_buf(),
        file_id: file_id.clone(),
        file_identity: row.file_identity.clone(),
        probe,
        generation: row.generation,
        rescan,
    };
    Ok(FilePreparation::Read(Box::new(PreparedFile { target, stored, row, detect_basis })))
    });
    super::run_policy::check()?;
    let prepared = prepared_result?;
    let PreparedFile {
        target,
        stored,
        mut row,
        mut detect_basis,
    } = match prepared {
        FilePreparation::Skipped(report, scanned) => return Ok((report, scanned)),
        FilePreparation::Read(prepared) => *prepared,
    };
    let file_id = target.file_id.clone();
    let probe = &target.probe;
    let mut retry = 0;
    let outcome = loop {
        control.check()?;
        super::run_policy::check()?;
        let mut limits = config.limits.clone();
        limits.jsonl.time_budget = limits
            .jsonl
            .time_budget
            .map(|budget| budget.saturating_sub(file_started.elapsed()));
        if limits
            .jsonl
            .time_budget
            .is_some_and(|budget| budget.is_zero())
        {
            return Err(CoreError::Interrupted("source_time_budget_exhausted"));
        }
        let scanned = adapter.scan(&target, &stored, &limits, config.now_ms);
        // File error isolation must not classify cancellation or another control stop as invalid data.
        super::run_policy::check()?;
        match scanned {
            Ok(outcome) => break outcome,
            Err(error) => {
                control.check()?;
                let Some(delay) = super::run_policy::retry_delay(
                    &error,
                    retry,
                    file_started.elapsed(),
                    config.limits.jsonl.time_budget,
                ) else {
                    return Err(error);
                };
                control.wait(delay)?;
                retry += 1;
            }
        }
    };
    access.with_storage(|storage| {
        super::run_policy::check()?;
        if !storage.conn().query_row(
            "SELECT enabled FROM source_instances WHERE instance_id=?1",
            [instance_id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(CoreError::Interrupted("source_disabled"));
        }
        // V30 fallback with structural diagnostics and no events/valid exclusive aggregate is incompatible.
        // Duplicate/OverlapUnknown reconciliation snapshots do not establish valid usage, such as old Codex without calls.
        // Keep prior results without new cursors/aggregates; retry after a parser update or explicit rescan.
        if let Some(DetectOutcome::Supported {
            format,
            format_version,
            basis,
        }) = adapter.scan_format(&outcome)
        {
            row.format_status = Some(format_status_json(
                &format,
                format_version.as_deref(),
                basis,
            ));
            detect_basis = Some(basis);
        }
        let compat_basis = detect_basis.or_else(|| {
            row.format_status
                .as_deref()
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                .and_then(|value| value.get("basis")?.as_str().map(str::to_string))
                .and_then(|basis| crate::domain::VersionBasis::parse(&basis).ok())
        });
        let fallback_failed = (stored.cursor.is_none() || target.rescan)
            && compat_basis == Some(crate::domain::VersionBasis::LatestFallback)
            && outcome.events.is_empty()
            && !outcome.aggregates.iter().any(|aggregate| {
                aggregate.coverage == crate::aggregates::Coverage::Exclusive
                    && aggregate.validate().is_ok()
            })
            && outcome.records_seen > 0
            && !outcome.diagnostics.is_empty();
        batch.events.extend(outcome.events.iter().cloned());
        batch
            .diagnostics
            .extend(outcome.diagnostics.iter().cloned());
        if !fallback_failed && (outcome.cursor.is_some() || outcome.parse_context.is_some()) {
            batch.checkpoints.push(CheckpointUpdate {
                scope_key: row.file_identity.clone(),
                cursor_value: outcome.cursor.clone(),
                parse_context: outcome.parse_context.clone(),
                source_revision: None,
            });
        }
        if !fallback_failed {
            aggregates.extend(outcome.aggregates);
        }
        reconciliations.extend(outcome.reconciliations.iter().cloned());
        row.len = probe.len;
        row.mtime_ms = probe.mtime_ms;
        row.head_hash = probe.head_hash;
        row.head_len = probe.head_len;
        row.tail_hash = probe.tail_hash;
        row.status = if fallback_failed {
            "incompatible".to_string()
        } else if outcome.health == "degraded" || outcome.status == ScanStatus::LineTooLong {
            "degraded".to_string()
        } else if compat_basis == Some(crate::domain::VersionBasis::LatestFallback)
            || outcome.health == "active_compat"
        {
            "active_compat".to_string()
        } else {
            "active".to_string()
        };
        file_updates.push(row.clone());
        let report_detail = if fallback_failed {
            Some("latest parser produced no validatable records; kept old results".to_string())
        } else if compat_basis == Some(crate::domain::VersionBasis::LatestFallback) {
            Some(format!(
                "latest_fallback: version compatibility unverified (found: {})",
                row.format_status
                    .as_deref()
                    .and_then(extract_found_version)
                    .unwrap_or_else(|| "missing".to_string())
            ))
        } else {
            None
        };
        let report_status = if fallback_failed {
            "incompatible".to_string()
        } else {
            outcome.status.as_str().to_string()
        };
        Ok((
            FileReport {
                file_id,
                status: report_status.to_string(),
                detail: report_detail,
                lines_read: outcome.lines_read,
                records_seen: outcome.records_seen,
                events: outcome.events.len() as u64,
                diagnostics: outcome.diagnostics.len() as u64,
            },
            true,
        ))
    })
}

/// Detection JSON in source_files.format_status (v3): selected metadata fields, without message content.
fn format_status_json(
    format: &str,
    found_version: Option<&str>,
    basis: crate::domain::VersionBasis,
) -> String {
    let compat = match basis {
        crate::domain::VersionBasis::KnownVersion => "verified",
        crate::domain::VersionBasis::LatestFallback => "unverified",
    };
    serde_json::json!({
        "format": format,
        "found_version": found_version,
        "basis": basis.as_str(),
        "compat": compat,
    })
    .to_string()
}

fn extract_found_version(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("found_version")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// File-stream identity: creation time and sampled prefix fingerprint/length, independent of path.
fn file_identity_of(probe: &FileProbe) -> String {
    format!(
        "file-{:x}-{:x}-{:x}",
        probe.created_ms.unwrap_or(-1),
        probe.head_hash,
        probe.head_len
    )
}
