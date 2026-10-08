//! Cline adapter; see architecture.md#adapter-layout.
//! - Stable entry point implements the shared interface and re-exports modules.
//! - detect checks legacy UI arrays and native SDK VS Code sessions independently.
//! - versions separates the legacy document format from SDK schema 1.
//! - common maps the legacy four exclusive token buckets and cost.
//!
//! SDK reference: 4.1.22 commit f58bc118 with checked real GUI/API/native samples.
//! Rewritable session metadata origin.version cannot identify all records; SDK uses latest_fallback.
//! Metrics can merge a run/retries; record observations without deriving underlying call counts.
//! Legacy reference: commit dcf8c3c33596e3d561a941202297c564a1cbcd49, A03,
//! source/documentation implementation awaiting native checks; local 2026-09-25 inventory was not_found.
//! - apps/vscode/src/shared/getApiMetrics.ts reads type="say" messages with
//!   say in {api_req_started, deleted_api_reqs, subagent_usage}.
//!   text is JSON with optional tokensIn/tokensOut/cacheWrites/cacheReads/cost;
//!   each field is checked with typeof number. api_req_started incorporates its
//!   matching api_req_finished; not every say message is a request. Four buckets are exclusive:
//!   getLastApiReqTotalTokens sums tokensIn + tokensOut + cacheWrites
//!   + cacheReads.
//!
//!   compaction tokensBefore/tokensAfter are SDK estimates around chars/4
//!   for the context display, excluded from usage.
//! - apps/vscode/src/core/storage/disk.ts writes tasks/<taskId>/
//!   ui_messages.json as a whole JSON array under host globalStorage.
//!   The VS Code extension identifier is saoudrizwan.claude-dev.

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_cline_usage, ClineUsage};
pub use detect::CLINE_FORMAT;
pub use versions::ui_messages_doc1::CLINE_PARSER_VERSION;
pub use versions::{
    ui_messages_doc1, CLINE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS,
};

/// VS Code extension globalStorage name; disk.ts uses host globalStorageFsPath for tasks.
pub const CLINE_EXT_GLOBAL_STORAGE: &str = "saoudrizwan.claude-dev";

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Strip UTF-8 BOM for detection/whole-file parsing; return bytes unchanged if no BOM.
fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes)
}

/// Stateless Cline adapter.
pub struct ClineAdapter;

impl Default for ClineAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClineAdapter {
    pub fn new() -> Self {
        ClineAdapter
    }
}

/// Legacy manual roots accept globalStorage containing tasks, or a directory named tasks.
fn cline_tasks_dir(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let tasks = root.join("tasks");
    if tasks.is_dir() {
        return Some(tasks);
    }
    if root.file_name()?.to_str()? == "tasks" {
        return Some(root.to_path_buf());
    }
    None
}

fn sdk_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    use versions::sdk_messages_v1::is_sdk_file;
    if root.is_file() {
        return if is_sdk_file(root) {
            vec![root.to_path_buf()]
        } else {
            vec![]
        };
    }
    let directory = [
        root.join("data").join("sessions"),
        root.join("sessions"),
        root.to_path_buf(),
    ]
    .into_iter()
    .find(|p| p.is_dir());
    directory
        .map(|p| crate::adapters::framework::enumerate_files_bounded(&p, 2, &is_sdk_file))
        .unwrap_or_default()
}

impl crate::adapters::framework::SourceAdapter for ClineAdapter {
    fn adapter_id(&self) -> &'static str {
        "cline"
    }

    fn agent(&self) -> &'static str {
        "cline"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        let env_path = |name: &str| {
            ctx.env
                .get(name)
                .filter(|v| !v.trim().is_empty())
                .map(|v| std::path::PathBuf::from(v.trim()))
        };
        let sdk_root = env_path("CLINE_SESSION_DATA_DIR")
            .or_else(|| env_path("CLINE_DATA_DIR").map(|p| p.join("sessions")))
            .or_else(|| env_path("CLINE_DIR").map(|p| p.join("data").join("sessions")))
            .or_else(|| {
                ctx.home_dir
                    .as_ref()
                    .map(|p| p.join(".cline").join("data").join("sessions"))
            });
        if let Some(root) = sdk_root {
            let basis = ["CLINE_SESSION_DATA_DIR", "CLINE_DATA_DIR", "CLINE_DIR"]
                .iter()
                .find(|key| env_path(key).is_some())
                .map(|key| RootBasis::EnvOverride((*key).into()))
                .unwrap_or(RootBasis::DefaultHome);
            roots.push((root, basis));
        }
        // VS Code globalStorage defaults: Windows %APPDATA%/Code, Unix XDG and macOS
        // ~/Library/Application Support/Code. APPDATA/XDG_CONFIG_HOME resolve host paths,
        // rather than defining Cline-specific environment overrides.
        if let Some(appdata) = ctx.env.get("APPDATA") {
            roots.push((
                std::path::PathBuf::from(appdata)
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(CLINE_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((
                home.join("Library")
                    .join("Application Support")
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(CLINE_EXT_GLOBAL_STORAGE),
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
                    .join(CLINE_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            // SDK files are discovered independently; legacy task roots use tasks itself or a tasks child.
            let mut files = sdk_files(&root);
            // Bounded depth 2 covers legacy tasks/<taskId>/ui_messages.json.
            if let Some(tasks) = cline_tasks_dir(&root) {
                files.extend(crate::adapters::framework::enumerate_files_bounded(
                    &tasks,
                    2,
                    &|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n == "ui_messages.json")
                            .unwrap_or(false)
                    },
                ));
            }
            files.sort();
            files.dedup();
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "cline@{}",
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
        if versions::sdk_messages_v1::is_sdk_file(&target.path) {
            return versions::sdk_messages_v1::scan(target, stored, limits, now_ms);
        }
        versions::ui_messages_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
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
                Availability::Partial(
                    "SDK VS Code 4.1.22 真实核对：inputTokens 含缓存，正值 reported、默认零 unknown；旧 UI 四桶仍文档级证据".into(),
                ),
                "SDK assistant.metrics 输入/输出/缓存；旧 UI say.text 四互斥桶独立映射",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(
                    "SDK 正缓存读已与真实 API 核对；默认零未知；旧 UI 文档级证据".into(),
                ),
                "SDK cacheReadTokens；旧 UI cacheReads",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(
                    "SDK 默认零未知，正缓存写待真实核对；旧 UI 文档级证据".into(),
                ),
                "SDK cacheWriteTokens；旧 UI cacheWrites",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(
                    "SDK metrics 是 usage_observation：可能合并 run/重试，不推导调用数；旧 UI 逐请求/聚合仍文档级证据".into(),
                ),
                "只有三类 usage 载体 say 计数；compaction/无 usage 的 say 不算请求",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(
                    "SDK 真实消息自身 modelInfo；旧 UI 无逐请求模型，不猜归属".into(),
                ),
                "SDK modelInfo.id/provider；不从 manifest 当前模型回填",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial("SDK 原生毫秒 ts 已核对，uncertain；旧 UI 文档级证据".into()),
                "消息 ts；SDK 不把消息时间认证为底层请求完成时间",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(
                    "SDK 未采到成本，保持 unknown；旧 UI 成本来源文档级证据，estimated".into(),
                ),
                "text.cost 浮点美元 → micro-USD（estimated）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("ui_messages.json 无逐次延迟字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "cline".to_string(),
            product: "Cline (VS Code 扩展)".to_string(),
            surfaces: vec!["vscode-extension".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "%APPDATA%/Code/User/globalStorage/saoudrizwan.claude-dev (Windows)",
                    "~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev (macOS)",
                    "${XDG_CONFIG_HOME:-~/.config}/Code/User/globalStorage/saoudrizwan.claude-dev (Linux)",
                    "~/.cline/data/sessions (SDK)"
                ],
                "env_override": ["CLINE_SESSION_DATA_DIR", "CLINE_DATA_DIR", "CLINE_DIR"],
                "manual_roots": true,
                "bounded": true,
                "pattern": "tasks/<taskId>/ui_messages.json 或 SDK sessions/<session>/<session>.messages.json",
                "profile": "SDK 只接已核对的 vscode/user/lead；CLI/其他 SDK 面不接入",
            }),
            detection: serde_json::json!({
                "magic": "旧 UI：数组 type/say；SDK：schema 1、vscode/user/lead、两处 sessionId 一致、assistant metrics",
                "version_field": "旧 UI 文档级 ui-messages-doc-1；SDK origin.version 为可重写 metadata，保持 latest_fallback",
                "fail_closed": true,
                "unknown_version": "未文档化 say 种类或非 say 记录类型：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "api_req_started（已与 api_req_finished 合并，final，task+say+ts 身份）",
                "sdk": "assistant metrics => usage_observation；默认零未知，可能合并 run/重试；源快照消失不撤销已观测用量",
                "aggregates": "deleted_api_reqs = 删除消息的用量聚合重述；subagent_usage = 子 Agent 批次聚合快照；各按 1 条 model_call 计（无法分解底层请求数）",
                "deleted_flow": "消息删除触发整写重写：消失的已入账键以 Corrected/Excluded 墓碑撤销旧贡献，防止与 deleted_api_reqs 聚合双计",
                "compaction": "say=compaction 的 tokensBefore/tokensAfter 是 SDK 估算，不进入用量（一次性诊断可见）",
                "streaming": "格式内无流式中间值（finished 合并落盘）",
                "subagent": "subagent_usage ⇒ sub_agent；与其明细任务文件无共享 ID 证据，跨文件不去重",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 已消费字节数（整写 JSON 全量重读，事件 upsert 幂等）",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）", "首采样变化（前缀改写）"],
                "budget": "单文件 32 MiB 有界读取；超限受限受控重试",
                "mid_write": "半程写入 parse 失败不推进游标，下轮确定性重试",
            }),
            dedup: serde_json::json!({
                "primary": "{taskId}:{say}:{ts}（实例命名空间；ts 是固定源码示例中唯一证据的消息身份字段）",
                "fallback": "无（缺 ts 记录跳过并记诊断，不用数组下标——删除流程会移位）",
                "deleted_flow": "消失键墓碑（Corrected/Excluded）；已墓碑键再现时 Corrected 优先，保持排除（记录限制）",
                "sdk_identity": "sessionId + message.id，JSON tuple 编码；只读原生 messages，不读 manifest/DB/hook 的第二份累计值",
                "cross_source": "CLI SDK / api_conversation_history.json 未接入；固定 VS Code 迁移转换不复制旧 UI metrics，迁移真实场景仍待验收",
            }),
            integrity: serde_json::json!({
                "success_only": "只统计带 usage 数字的载体；无 usage 的 api_req_started（未完成请求）无事件、部分可用，其余记录继续入账",
                "hidden_calls": "deleted/subagent 聚合是可见用量；标题生成等辅助调用无证据，未观测",
                "sampling": "上游 JSON parse 失败静默忽略；本适配器逐条记诊断后跳过",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（type/say/ts/text 内 tokensIn/tokensOut/cacheWrites/cacheReads/cost），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": "cline-ui-doc1+sdk-v1",
                "format_evidence": "SDK VS Code 4.1.22 固定 f58bc118 真实 GUI/API/原生三条核对；旧 UI 固定 dcf8c3c 文档级证据，待真实样本",
                "upgrade_policy": "say 种类/记录类型/字段形状偏离 fail closed；取得真实样本后扩展接受集与逐请求模型归属",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "无变化探测短路；有变化全量重读（32 MiB 有界）+ upsert 幂等",
                "pause_cancel": "文件间可停；单文件读取有界",
            }),
            limitations: vec![
                "真实核对仅覆盖 VS Code 4.1.22 LM Studio 配置路线/本地兼容 API 的 SDK 主会话；其他 provider、迁移、辅助和重试路径未真实核对".into(),
                "SDK origin.version 不认证消息所属版本；默认零未知，metrics 可合并 run/重试，调用数保持 unknown".into(),
                "ui_messages.json 实际消息词汇（ask、say=text/tool 等非用量种类）未在固定源码中枚举：按未文档化处理 fail closed，待真实样本扩展接受集".into(),
                "deleted_api_reqs/subagent_usage 聚合无法分解底层请求数：call_count 按聚合记录计（下限），token 值为聚合真值".into(),
                "subagent_usage 聚合与其明细任务文件（若存在）无共享 ID 证据，跨文件不去重，真实样本需核验是否双源".into(),
                "cost 来源口径（扩展自算价目 or 供应商账单）未证实：按 micro-USD estimated 入账，不与远端账单相加".into(),
                "旧 UI 逐请求模型字段未证实：model 保持 unknown；SDK 只用自身 modelInfo".into(),
                "消息删除后再次原样恢复的键已被墓碑排除，重新出现时保持排除（Corrected 优先）".into(),
                "整写 JSON 每次变化全量重读（32 MiB 有界），成本随文件大小线性；靠 upsert 幂等保证不双计".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}
