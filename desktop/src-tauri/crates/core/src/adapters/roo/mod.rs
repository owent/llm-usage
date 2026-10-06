//! Roo Code 适配器（VS Code 扩展 + CLI；独立目录约定）。载体：
//! globalStorage `RooVeterinaryInc.roo-cline/tasks/<taskId>/ui_messages.json`。
//! 官方源码已归档（b867ec9，2026-05）；tokensIn 含缓存（与 cline 四桶互斥
//! 字段语义的产品间差异已在 adapters.md 登记）。3.54.0 官方 VSIX 另有
//! 真实 extension-host/API 样本：默认零未知，取消及缓存详情覆盖缺口保留。

pub mod detect;
pub mod versions;

pub use detect::ROO_FORMAT;
pub use versions::ui_messages_doc1;
pub use versions::{LATEST_IMPL_ID, ROO_FORMAT_VERSION, VERIFIED_VERSION_IMPLS};

/// VS Code 扩展 globalStorage 目录名（小写扩展 ID）。
pub const ROO_EXT_GLOBAL_STORAGE: &str = "rooveterinaryinc.roo-cline";
/// CLI 缺省任务存储（vscode-shim：~/.vscode-mock/global-storage）。
pub const ROO_CLI_STORAGE_DIR: &str = ".vscode-mock";

/// Roo Code 适配器（无状态）。
pub struct RooAdapter;

impl Default for RooAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RooAdapter {
    pub fn new() -> Self {
        RooAdapter
    }
}

/// 手工根兼容：globalStorage 目录、tasks 目录或任务目录本身。
fn roo_tasks_dir(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let tasks = root.join("tasks");
    if tasks.is_dir() {
        return Some(tasks);
    }
    if root.file_name()?.to_str()? == "tasks" {
        return Some(root.to_path_buf());
    }
    None
}

impl crate::adapters::framework::SourceAdapter for RooAdapter {
    fn adapter_id(&self) -> &'static str {
        "roo"
    }

    fn agent(&self) -> &'static str {
        "roo-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // APPDATA is an independent Windows default; isolated callers can omit
        // home without disabling this root or visiting unrelated fixed homes.
        if let Some(appdata) = ctx.env.get("APPDATA") {
            roots.push((
                std::path::PathBuf::from(appdata)
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(ROO_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
        }
        if let Some(home) = &ctx.home_dir {
            // CLI（vscode-shim 固定 globalStorage，无 publisher 层）。
            roots.push((
                home.join(ROO_CLI_STORAGE_DIR).join("global-storage"),
                RootBasis::DefaultHome,
            ));
            // VS Code 扩展 globalStorage 三平台默认 + .vscode-server 远端变体
            //（远端在 WSL/SSH 侧，本机探测通常为空，保留候选）。
            roots.push((
                home.join("Library")
                    .join("Application Support")
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join(ROO_EXT_GLOBAL_STORAGE),
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
                    .join(ROO_EXT_GLOBAL_STORAGE),
                RootBasis::DefaultHome,
            ));
            roots.push((
                home.join(".vscode-server")
                    .join("data")
                    .join("User")
                    .join("globalStorage")
                    .join(ROO_EXT_GLOBAL_STORAGE),
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
            let Some(tasks) = roo_tasks_dir(&root) else {
                continue;
            };
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
            "roo@{}",
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
        let note = "官方源码/VSIX 3.54.0（27001b2b）与独立容器真实 VS Code/API 对照已核验；仅该 OpenAI-compatible 样本范围"
            .to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "api_req_started 正 tokensIn 含缓存、正 tokensOut 输出；四桶默认零未知；正总输入/输出可派生 total，两缓存均已知才推导未缓存",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "正 cacheReads 为 tokensIn 子集；默认零未知，OpenAI-compatible 未读取 prompt_tokens_details.cached_tokens 的真实覆盖缺口保留",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "正 cacheWrites 为 tokensIn 子集，默认零未知",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(Availability::Partial(note.clone()), "每条合并后的 api_req_started 一次请求；LIFO 配对（当前版本 finished 已不写，legacy 兼容）"),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Unavailable("api_req_started 无模型字段；tokscale 从 api_conversation_history 的 XML 标签提取是启发式，不采用（与 cline/zoo 同限制）".into()),
                "无（customStoragePath/modelInfo 均非逐请求模型载体）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "api_req_started ts（请求起点）；condense ts 为写入时刻（uncertain）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(Availability::Partial(note.clone()), "正 cost 为扩展按费率自算的 estimated micro-USD；缺价目默认零未知；condense_context.contextCondense.cost 同口径"),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟字段".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "roo".to_string(),
            product: "Roo Code（RooVeterinaryInc.roo-cline，Cline 血统独立产品；仓库已归档）".to_string(),
            surfaces: vec!["vscode".into(), "cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "Windows %APPDATA%/Code/User/globalStorage/rooveterinaryinc.roo-cline",
                    "macOS ~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline",
                    "unix ~/.config/Code/User/globalStorage/rooveterinaryinc.roo-cline",
                    "~/.vscode-mock/global-storage（CLI，vscode-shim）",
                    "~/.vscode-server/data/User/globalStorage/rooveterinaryinc.roo-cline（远端变体候选）",
                ],
                "env_override": null,
                "manual_roots": "含 tasks 子目录按 globalStorage 根解析；roo-code.customStoragePath 用户需手工添加",
                "bounded": true,
                "pattern": "tasks/<taskId>/ui_messages.json（深度 2；history_item.json/_index.json 是汇总索引不读）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "文件头 64 KiB 以 JSON 数组开头 + type/say 指纹",
                "version_field": "无；文档级锚点 roo-ui-messages-doc-1（固定源码 b867ec9 完整 say/ask 枚举）",
                "registry": "adapters/roo/versions 注册表（唯一条目；与 cline 目录独立）",
                "fail_closed": true,
                "unknown_version": "未文档化 type/kind 整文件拒绝，待真实样本扩展",
            }),
            fields,
            lifecycle: serde_json::json!({
                "consolidation": "api_req_started/finished LIFO 配对（finished 覆盖同名字段；当前版本不写 finished，legacy 兼容）",
                "deleted_memo": "api_req_deleted / deleted_api_reqs（旧 Cline）是已扣除量备忘：不计入（上游 consolidateTokenUsage 同口径）",
                "condense_context": "contextCondense.cost 是压缩摘要独立调用总成本：按辅助调用入账（token 未知）",
                "subtask": "子任务独立目录独立计费；父任务 subtask_result 仅文本摘要",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON 32 MiB 上限：字节游标 + generation 重扫；事件键 {task}:{say}:{ts}:{seq} upsert 幂等；doc1→2 触发未变化游标重评",
            }),
            dedup: serde_json::json!({
                "primary": "{taskId}:api_req_started:{ts}:{seq} / {taskId}:condense_context:{ts}:{seq}",
                "cross_source": "与 cline/zoo 数据根不重叠；customStoragePath 根需手工添加避免漏采",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "消息删除（无 checkpoint）直接截断不产生备忘；已入账事件保持（删除重述语义待真实样本）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::ui_messages_doc1::ROO_PARSER_VERSION,
                "format_evidence": "Roo-Code b867ec9 结构锚点；3.54.0/27001b2b Task.ts/cost.ts/OpenAI provider 与官方 VSIX 两组真实载体/API",
                "evidence_level": "official-source + official-distribution + real-local-model（VS Code extension host）",
                "upgrade_policy": "未变化旧游标重评；只有完整旧摘要匹配已核验零默认/派生修正，保留首次观察、诊断与真实冲突",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "tokensIn 含缓存口径由官方源码及真实 API/原生对照核验；不替代 Cline 独立版本验证".into(),
                "无逐请求模型字段（XML 标签提取启发式不采用）".into(),
                "customStoragePath/远端 .vscode-server 根需手工添加".into(),
                "默认取消场景 API 三次、原生两次，缺失调用不补造；OpenAI-compatible 缓存详情丢失，不将零当无缓存".into(),
                "仅 3.54.0 官方 VSIX 的本地 provider 样本；任务未成功完成，CLI、其他 provider/版本及子代理/工具场景待验".into(),
            ],
        }
    }
}
