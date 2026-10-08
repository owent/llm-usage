//! Zoo Code adapter layout: architecture.md#adapter-layout.
//! - Stable entry point for the shared interface and reexports.
//! - detect checks documented product/format fingerprints; ui_messages.json has
//!   no client-version field for dispatch.
//! - versions keeps one documented zoo-ui-messages-doc-1 format entry,
//!   independently of Cline's implementation.
//! - common contains Zoo-specific total-input/cache and cost mapping.
//!
//! Native reference: official VSIX 3.86.0 / 6aa9d0174a9ecae155c6c5db9134bead4b67197d.
//! Nonempty isolated VS Code container samples were compared with the public API/model service 2026-10-06.
//! Older f780647 finished/condense paths remain separately documented without native scenario acceptance.
//! - packages/core/src/message-utils/consolidateTokenUsage.ts reads say/api_req_started
//!   text JSON with optional tokensIn/tokensOut/cacheWrites/cacheReads/cost
//!   and apiProtocol.
//!   tokensIn includes cache; contextCondense.cost contributes to totalCost.
//!   Upstream calculates contextTokens=input+output.
//! - packages/core/src/message-utils/consolidateApiRequests.ts pairs finished
//!   with the latest unmatched started in LIFO order and merges text JSON.
//!   Finished fields override start fields; unmatched finished entries are discarded.
//! - packages/core/src/task-persistence/taskMessages.ts and
//!   src/shared/globalFileNames.ts store full JSON arrays under host globalStorage
//!   at tasks/<taskId>/ui_messages.json.
//! - VS Code extension: ZooCodeOrganization.zoo-code (src/package.json).
//!   Source CLI default: ~/.vscode-mock/global-storage (apps/cli and vscode-shim).
//!
//! Verify Zoo's own directories and fields rather than inheriting Roo rules (A19).

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_zoo_cost, map_zoo_usage, ZooUsage};
pub use detect::ZOO_FORMAT;
pub use versions::ui_messages_doc1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS, ZOO_FORMAT_VERSION};

/// Lowercase extension ID from package publisher/name used as the VS Code
/// globalStorage directory name.
pub const ZOO_EXT_GLOBAL_STORAGE: &str = "zoocodeorganization.zoo-code";
/// Source CLI task-history DEFAULT_CLI_TASK_STORAGE_PATH and vscode-shim default:
/// ~/.vscode-mock/global-storage.
pub const ZOO_CLI_STORAGE_DIR: &str = ".vscode-mock";

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Strip UTF-8 BOM for detection and whole-file parsing; return other bytes unchanged.
fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes)
}

/// Stateless Zoo Code adapter.
pub struct ZooAdapter;

impl Default for ZooAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZooAdapter {
    pub fn new() -> Self {
        ZooAdapter
    }
}

/// Manual roots may contain tasks/ or identify the tasks directory itself.
fn zoo_tasks_dir(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let tasks = root.join("tasks");
    if tasks.is_dir() {
        return Some(tasks);
    }
    if root.file_name()?.to_str()? == "tasks" {
        return Some(root.to_path_buf());
    }
    None
}

impl crate::adapters::framework::SourceAdapter for ZooAdapter {
    fn adapter_id(&self) -> &'static str {
        "zoo"
    }

    fn agent(&self) -> &'static str {
        "zoo-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // APPDATA supplies an independent Windows default, including isolated callers
        // that omit HOME/USERPROFILE.
        if let Some(appdata) = ctx.env.get("APPDATA") {
            roots.push((
                std::path::PathBuf::from(appdata)
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(ZOO_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
        }
        // Source CLI default is ~/.vscode-mock/global-storage
        // under the user home.
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join(ZOO_CLI_STORAGE_DIR).join("global-storage"),
                RootBasis::DefaultHome,
            ));
            // VS Code extension roots: Windows APPDATA/Code, macOS
            // ~/Library/Application Support/Code, and Unix XDG or ~/.config/Code.
            // APPDATA/XDG_CONFIG_HOME select the platform's default base directory.
            roots.push((
                home.join("Library")
                    .join("Application Support")
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(ZOO_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
            let config = ctx
                .env
                .get("XDG_CONFIG_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            roots.push((
                config
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(ZOO_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // A manual root can be tasks/; standard storage roots contain a tasks/ child.
            let Some(tasks) = zoo_tasks_dir(&root) else {
                continue;
            };
            // Enumerate tasks/<taskId>/ui_messages.json within a bounded depth of two.
            let files = crate::adapters::framework::enumerate_files_bounded(&tasks, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "ui_messages.json")
                    .unwrap_or(false)
            });
            if !files.is_empty() && seen.insert(tasks.clone()) {
                out.push(DiscoveredRoot {
                    root: tasks,
                    basis,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "zoo@{}",
            crate::adapters::framework::normalize_path(&root.root)
        )
    }

    fn detect(
        &self,
        path: &std::path::Path,
    ) -> Result<crate::adapters::framework::DetectOutcome, crate::error::CoreError> {
        detect::detect(path)
    }

    fn scan(
        &self,
        target: &crate::adapters::framework::ScanTarget,
        stored: &crate::adapters::framework::StoredScanState,
        limits: &crate::adapters::framework::ScanLimits,
        now_ms: i64,
    ) -> Result<crate::adapters::framework::ScanOutcome, crate::error::CoreError> {
        versions::ui_messages_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting = "官方 VSIX/固定源码 3.86.0（6aa9d017）与独立容器 VS Code/API/原生文件已核对；仅该 OpenAI-compatible 样本范围".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, note: &str| {
            serde_json::json!({
                "availability": availability,
                "note": note,
            })
        };
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "api_req_started text 四字段（tokensIn/tokensOut/cacheWrites/cacheReads）；tokensIn 含缓存（两协议同口径，固定源码注释），cache 为其子集、input_uncached 不拆",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "正 cacheReads reported（tokensIn 子集）；默认零未知；OpenAI-compatible 未读嵌套 cached_tokens",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "正 cacheWrites reported（tokensIn 子集）；默认零未知",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "每条合并后的 api_req_started 一次请求（consolidateApiRequests LIFO 配对；不能每条 say 算请求）；unpaired started 无数字不入账",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Unavailable(
                    "api_req_started text 与 condense_context 均无模型字段（固定源码类型 ParsedApiReqStartedTextType）".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "api_req_started ts 是请求起点（source_start；cost 由 finished 合并写回）；condense ts 是消息写入时刻（uncertain）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "正 cost 为扩展估价；当前版本内联更新 started，旧 finished 合并仍支持；默认零未知；condense_context 同口径",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("usage 载体无延迟/TTFT 字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "zoo".to_string(),
            product: "Zoo Code（Zoo-Code-Org，Roo 血统独立产品）".to_string(),
            surfaces: vec!["vscode".into(), "cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "~/.vscode-mock/global-storage（CLI 缺省，固定源码）",
                    "Windows %APPDATA%/Code/User/globalStorage/zoocodeorganization.zoo-code",
                    "macOS ~/Library/Application Support/Code/User/globalStorage/zoocodeorganization.zoo-code",
                    "unix ~/.config/Code/User/globalStorage/zoocodeorganization.zoo-code",
                ],
                "env_override": null,
                "manual_roots": "含 tasks 子目录按 globalStorage 根解析，否则按 tasks 目录本身",
                "bounded": true,
                "pattern": "tasks/<taskId>/ui_messages.json（整写 JSON 数组；history_item.json/_index.json 只用于上游会话索引，不读）",
                "profile": "无 profile 概念（固定源码未见）",
            }),
            detection: serde_json::json!({
                "magic": "文件头 64 KiB 以 JSON 数组开头 + type/say 指纹",
                "version_field": "无版本字段（整写 JSON 数组）；文档级锚点 zoo-ui-messages-doc-1",
                "registry": "adapters/zoo/versions 注册表（唯一条目：文档级锚点；与 cline 目录独立）",
                "fail_closed": true,
                "unknown_version": "无逐请求客户端版本；格式锚点不认证其他版本；未知 ask/say/类型扫描层 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "consolidation": "consolidateApiRequests LIFO 配对：finished text 并入最近 started（finish 覆盖 start）；unpaired finished 丢弃（上游算法同款）",
                "condense_context": "contextCondense.cost 计入上游 totalCost ⇒ 按辅助调用入账（token 全未知，cost 有则映射）；newContextTokens 是上下文规模不入账",
                "no_double_count": "api_req_finished 本身不产事件（usage 已并入 started）；condense 与 api_req 载体互斥",
                "unfinished": "unpaired started 无 token 数字 ⇒ 无 token 证据不产事件（下轮 finished 合并后入账）",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON：已消费字节数复用框架无变化短路；改写/截断走 generation 重扫",
                "whole_file_parse": "每轮全量有界读取（32 MiB）；事件按 {task}:{say}:{ts} 稳定身份 upsert 幂等",
                "half_line": "半程写入 parse 失败不推进游标，下轮确定性重试",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            dedup: serde_json::json!({
                "primary": "{taskId}:api_req_started:{ts} / {taskId}:condense_context:{ts}",
                "same_ms": "同任务同 say 同毫秒碰撞未观测（低频载体）；上游无消息 ID，键按 cline 同型，限制如实标注",
                "cross_source": "api_conversation_history.json/task_metadata.json/history_item.json 不读；与 cline/roo 数据根不重叠",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "未落盘/被删消息不可见（消息删除流程未在固定源码文档化：不推导墓碑，已入账事件保持）；不补零",
                "sampling": "未观测到采样；坏 text 逐条隔离记诊断",
                "api_protocol": "apiProtocol ∈ {anthropic, openai} 仅作字段证据：两协议 tokensIn 均含缓存（固定源码注释），不做分支",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::ui_messages_doc1::ZOO_PARSER_VERSION,
                "format_evidence": "A19 f780647 + 3.86.0/6aa9d017 完整类型枚举、Task 内联 writer 与 OpenAI codec；官方 VSIX/真实 API/原生文件对照",
                "evidence_level": "real-container（3.86.0 VS Code OpenAI-compatible；其他路径保留文档级证据）",
                "upgrade_policy": "完整旧摘要/旧游标重评默认零，保留真实冲突、观察时间与历史；未知类型 fail closed",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "无变化字节短路；变化文件全量重读（32 MiB 上界）",
                "pause_cancel": "文件间可停；单文件解析有界",
            }),
            limitations: vec![
                "真实样本仅 3.86.0 VS Code OpenAI-compatible 一次请求；CLI、其他协议、压缩/删除等路径未实测".into(),
                "已验证非用量 ask/say 跳过；未知枚举仍整文件拒绝、不推进游标".into(),
                "默认零缓存/费用未知；OpenAI-compatible 忽略嵌套 cached_tokens；未落盘或删除调用不补造".into(),
                "tokensIn 含缓存的口径来自固定源码注释：cacheReads/cacheWrites 报告为子集，input_uncached 与互斥分解不做".into(),
                "无逐请求模型字段（ParsedApiReqStartedTextType 无 model）：模型维度不可用".into(),
                "消息删除流程未文档化：无墓碑推导，已入账事件在源文件改写后保持（删除重述语义待真实样本）".into(),
                "JetBrains 变体缺证据后移 F1（A19）；CLI 与 VS Code 表面共用同一任务文件格式（按固定源码），各表面真实布局待样本".into(),
            ],
        }
    }
}
