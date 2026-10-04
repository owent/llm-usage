//! 适配器框架：discover → detect → scan → map → capability 统一接口，
//! 以及把扫描结果接入 M1 `commit_batch` 管线的运行器（V12：重复扫描不增量）。
//!
//! 规则要点：
//! - discover：候选路径 + 环境覆盖 + 手工根，有界枚举，不全盘扫描；
//! - detect：文件 magic/记录类型/schema 指纹 + Agent 目录版本注册表分派；
//!   未知/缺失版本默认尝试该 Agent 最新内置解析器并带兼容标记（V17/V30），
//!   已确认不兼容的版本与未知格式 fail closed 返回受限；
//! - scan：增量游标读取（JSONL 游标 = 文件身份 + generation + 完整行字节偏移 + 解析上下文）；
//! - capability：每源字段能力声明，供未来数据源页使用；
//! - 诊断只存字段名/错误码/位置，不复制原始内容。

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

/// 单源每轮墙钟超时初值（architecture.md：30 秒）。
pub const DEFAULT_SOURCE_TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
/// 发现阶段的文件/目录上界（有界探测）。
pub const DISCOVER_MAX_FILES: usize = 20_000;
pub const DISCOVER_MAX_DIRS: usize = 50_000;

/// detect 入口的瞬态 IO 错误判定：杀软/产品进程短暂持锁
/// （Windows 共享违例 → PermissionDenied）、超时、枚举后文件被清理。
/// 瞬态错误必须 Pending 下轮重探，不能固化为扫描失败或 UnknownFormat。
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

/// detect 文件头读取助手：`Ok(None)` = 瞬态不可读（调用方返回 Pending）；
/// 硬错误上抛 CoreError。
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

/// 发现上下文。环境变量以显式 map 传入，便于测试且不依赖真实进程环境。
#[derive(Debug, Clone, Default)]
pub struct DiscoverContext {
    pub home_dir: Option<PathBuf>,
    pub env: std::collections::BTreeMap<String, String>,
    pub manual_roots: Vec<PathBuf>,
}

/// 候选根的来源依据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootBasis {
    /// 环境变量覆盖（值是变量名，不是内容）。
    EnvOverride(String),
    /// 平台默认用户目录。
    DefaultHome,
    /// 用户手工添加的根。
    Manual,
}

/// 一个有界枚举后的候选根。
#[derive(Debug, Clone)]
pub struct DiscoveredRoot {
    pub root: PathBuf,
    pub basis: RootBasis,
    pub files: Vec<PathBuf>,
}

/// 格式探测结果（architecture.md 未知版本兼容约定）：
/// - Supported：Agent 身份与输入类型已确认。已知版本按注册表映射分派（KnownVersion）；
///   未知/缺失版本默认选择该 Agent 最新内置解析器（LatestFallback），结果须带兼容标记。
/// - UnsupportedVersion：有证据判定不兼容的版本（如固定源码证实格式不同），不尝试回退。
/// - UnknownFormat：Agent 身份或格式无法确认，fail closed；不得返回“成功 0 条”（V17）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectOutcome {
    Supported {
        format: String,
        /// 来源原始版本；版本字段缺失时为 None（仍可 LatestFallback）。
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
    /// 文件尚无可判定内容（如新建空文件）：下轮重探。
    Pending,
}

/// 单项能力可用性。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Partial(String),
    Unavailable(String),
}

/// 结构化能力声明（发现/探测/映射/增量/去重/完整性/维护/定时/限制）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityTable {
    pub adapter_id: String,
    pub product: String,
    pub surfaces: Vec<String>,
    pub supported_versions: Vec<String>,
    pub discovery: serde_json::Value,
    pub detection: serde_json::Value,
    /// 字段能力：token/缓存读/缓存写/逐次请求/模型/时间/费用/延迟。
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub lifecycle: serde_json::Value,
    pub incremental: serde_json::Value,
    pub dedup: serde_json::Value,
    pub integrity: serde_json::Value,
    pub maintenance: serde_json::Value,
    pub scheduling: serde_json::Value,
    pub limitations: Vec<String>,
}

/// 逐文件扫描目标。
#[derive(Debug, Clone)]
pub struct ScanTarget {
    pub instance_id: String,
    /// 规范化后的文件路径（只进本地库，不进对外输出）。
    pub path: PathBuf,
    /// source_files.file_id（规范化路径字符串）。
    pub file_id: String,
    /// source_files.file_identity / checkpoint scope_key（跨改名稳定）。
    pub file_identity: String,
    /// 探测到的当前文件状态。
    pub probe: FileProbe,
    /// 代数裁决后的 generation（重扫时已是新值）。
    pub generation: i64,
    /// 本次是否从头重扫（替换/截断/重建）。
    pub rescan: bool,
}

/// 已存储的扫描状态（游标 + 解析上下文）。
#[derive(Debug, Clone, Default)]
pub struct StoredScanState {
    pub cursor: Option<serde_json::Value>,
    pub parse_context: Option<serde_json::Value>,
}

/// 扫描限制。
#[derive(Debug, Clone, Default)]
pub struct ScanLimits {
    pub jsonl: JsonlLimits,
}

/// 逐文件扫描状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanStatus {
    /// 读到当前文件尾。
    Complete,
    /// 达到读取上限或超时，游标停在完整行边界，下轮继续。
    BudgetExhausted,
    /// 某行超过单行上限：受限，游标停在该行起点，允许受控重试。
    LineTooLong,
    /// 空文件等待内容。
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

/// 快照对账结果（白名单数值，无正文）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconciliation {
    /// 系列标签（如 "session_cumulative_snapshot"），不含 ID/路径。
    pub series: String,
    /// 逐次明细的 total_tokens 合计。
    pub detail_sum: i64,
    /// 最终累计快照值。
    pub snapshot_final: Option<i64>,
    /// 按已核验规则排除的边界记录合计（如 compaction 携带记录）。
    pub carried_sum: i64,
    /// detail_sum - (snapshot_final + carried_sum)。
    pub difference: Option<i64>,
    /// matched / mismatch / no_snapshot。
    pub verdict: String,
}

/// 一次扫描的产出。
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
    /// 源文件健康（active / degraded）。
    pub health: String,
}

/// 适配器统一接口。
pub trait SourceAdapter: Send + Sync {
    fn adapter_id(&self) -> &'static str;
    /// 统计归属的 Agent 名（事件 agent 字段）。
    fn agent(&self) -> &'static str;
    /// 有界发现：候选路径 + 环境覆盖 + 手工根；不全盘扫描。
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot>;
    /// 由候选根得到稳定的本机实例 ID。
    fn instance_id(&self, root: &DiscoveredRoot) -> String;
    /// 格式探测：magic/记录类型/schema 指纹；未知版本 fail closed。
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError>;
    /// 增量扫描一个已支持的文件。
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError>;
    /// 重新检查已消费且字节未变化的文件，同时保留游标上下文中的单调修订。
    /// 默认继续跳过；快照适配器可用于一次性的统计/健康规则修正。
    fn should_scan_unchanged(&self, _stored: &StoredScanState) -> bool {
        false
    }
    /// 结构化能力声明。
    fn capability(&self) -> CapabilityTable;

    /// A verified authoritative archive can replace file scanning for this root.
    /// Errors must remain visible; never silently add a second carrier's counts.
    fn scan_archive(
        &self,
        _storage: &Storage,
        _root: &DiscoveredRoot,
        _config: &RunConfig,
    ) -> Result<Option<BatchOutcome>, CoreError> {
        Ok(None)
    }
}

/// 规范化路径字符串（统一分隔符并去除 Windows verbatim 前缀；仅用于本地身份）。
pub fn normalize_path(path: &Path) -> String {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").map(str::to_string).unwrap_or(s)
}

/// 有界目录枚举：深度受限、不跟随符号链接、文件数/目录数有上限。
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

/// source_instances 注册输入。enabled 等用户设置在冲突更新时保留。
/// origin_host_id：本机核验采集传入本地主机 ID；None 表示未区分（legacy_unknown）。
/// 已属于其他主机的来源不被覆盖；legacy_unknown 来源可被本机核验采集认领
/// （文件就在本机且 locality 已核验 = 可证明映射，data-contract.md#provenance）。
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
    /// 来源归属主机（M1a）；None → legacy_unknown 命名空间。
    pub origin_host_id: Option<String>,
}

/// 迁移前的历史来源命名空间（v4 前未记录主机身份）。
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

/// source_files 行（注册与改名/替换探测用）。
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
    /// 探测结论（JSON）：原始版本、所选格式/parser、选择依据、兼容状态（v3 列）。
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

/// 按文件身份查找（改名探测：内容流身份稳定，路径可变）。
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

/// 读取检查点（游标 + 解析上下文）。
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

/// 从 source_files 行构造 [`StoredFileState`]（代数裁决输入）。
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

/// 一次源采集运行的配置。
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub timezone: String,
    pub now_ms: i64,
    pub limits: ScanLimits,
    pub trigger: TriggerKind,
    /// 运行/批次 ID 前缀；实际 ID 追加实例序号。
    pub run_id_prefix: String,
    /// 采集归属的本机来源主机 ID（M1a）；None 表示未区分（legacy_unknown）。
    /// 由应用层经 `Storage::ensure_local_host` 取得后传入。
    pub origin_host_id: Option<String>,
}

/// 逐文件运行报告。
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

/// 逐实例运行报告。
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

/// 运行一个适配器：发现 → 逐文件探测/扫描 → 单事务提交（V12 语义）。
/// 返回逐实例报告；单文件失败不中断其他文件，实例提交失败标 failed。
pub fn run_adapter_scan(
    storage: &Storage,
    adapter: &dyn SourceAdapter,
    ctx: &DiscoverContext,
    config: &RunConfig,
) -> Result<Vec<SourceRunReport>, CoreError> {
    run_adapter_scan_filtered(storage, adapter, ctx, config, &InstanceFilter::default())
}

/// 实例过滤（逐源定时：自定义计划的来源只按自身节奏触发，
/// 全局刷新排除它们；到期刷新只包含到期实例）。
#[derive(Debug, Clone, Default)]
pub struct InstanceFilter {
    /// 仅这些实例参与（None = 不限）。
    pub include: Option<BTreeSet<String>>,
    /// 这些实例跳过（None = 不排除）。
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

/// 带实例过滤的运行（run_adapter_scan 的过滤版；逐源定时接线用）。
/// 停用实例（source_instances.enabled=0）在统一门控，任何触发路径
/// （Interval/Startup/Manual/FixedTime）都不读取——"只读取用户启用的
/// 本地来源"与 set_source_enabled"停用后不再读取该实例"的约定。
/// 停用集加载失败时本适配器 fail-closed（宁可不扫，不越权读取）。
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
    fn capability(&self) -> CapabilityTable {
        self.adapter.capability()
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

/// Metadata and commits share the existing writer; carrier parsing holds no lock.
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
    // 根去重（环境覆盖/默认/手工可能指向同一目录；Windows 大小写别名先归一）。
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
            // 本次未执行扫描；不能把合并请求报告为已成功完成。
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
                adapter.scan_archive(storage, root, config)
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
        for path in &root.files {
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
        // 无任何文件变化时跳过提交（无变化扫描不推进修订号）。
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
                // 原生汇总与事件、游标一起提交；任一失败均可完整重放。
                for aggregate in &aggregates {
                    super::run_policy::check()?;
                    crate::aggregates::upsert_source_aggregate_tx(&tx, aggregate, config.now_ms)?;
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

/// 扫描单个文件：注册 → 代数裁决 → （必要时）探测 → 增量扫描 → 累积进批次。
/// 返回（文件报告， 是否发生了实际读取）。
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
    // 注册/改名探测：路径未命中时按身份命中（同一内容流改名不算新文件）。
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
    // 无变化短路：全部字节已消费且代数连续。
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
    // 首次或重扫时重新探测格式；已确认不兼容的版本与未知格式 fail closed（V17），
    // 未知/缺失版本按该 Agent 注册表选择最新内置解析器并带兼容标记。
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
        detect_basis,
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
        // Carrier error isolation must not turn a control stop into bad data.
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
        // 未知版本兼容尝试的失败判定（V30）：读到记录、零事件且带结构诊断 ⇒ 判不兼容，
        // 保留旧结果、不提交事件/游标/聚合；下轮解析器更新或显式重扫可重新尝试。
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

/// 探测结论 JSON（source_files.format_status，v3 列）：白名单字段，无正文。
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

/// 文件内容流身份：创建时间 + 首采样指纹（追加稳定，不依赖路径）。
fn file_identity_of(probe: &FileProbe) -> String {
    format!(
        "file-{:x}-{:x}-{:x}",
        probe.created_ms.unwrap_or(-1),
        probe.head_hash,
        probe.head_len
    )
}
