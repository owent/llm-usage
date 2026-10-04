//! Cline 适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测（文档级指纹；ui_messages.json 无版本字段，
//!   不做版本分派）；
//! - [`versions`]：统一形状的格式注册表（唯一条目：文档级 ui-messages-doc-1）；
//! - 产品特有映射在 [`common`]（四互斥桶 + cost）。
//!
//! 原始格式依据（固定源码 dcf8c3c33596e3d561a941202297c564a1cbcd49，A03，
//! 按文档或源码实现，待真实样本核验；本机 2026-09-25 盘点 not_found）：
//! - `apps/vscode/src/shared/getApiMetrics.ts`：usage 载体是 type="say" 且
//!   say ∈ {api_req_started, deleted_api_reqs, subagent_usage} 的消息，
//!   `text` 为 JSON 字符串，字段 tokensIn/tokensOut/cacheWrites/cacheReads/cost
//!   逐字段可选（typeof number 检查）；api_req_started 已与对应
//!   api_req_finished 合并；不能每条 say 算请求。四桶互斥：
//!   getLastApiReqTotalTokens 的 total = tokensIn + tokensOut + cacheWrites
//!   + cacheReads。
//!
//!   say="compaction" 的 tokensBefore/tokensAfter 是 SDK 估算（chars/4 级），
//!   只驱动上下文条显示，不进入用量。
//! - `apps/vscode/src/core/storage/disk.ts`：任务目录 = 宿主 globalStorage 下
//!   `tasks/<taskId>/`，消息持久化为 `ui_messages.json`（整写 JSON 数组）；
//!   宿主为 VS Code 扩展 saoudrizwan.claude-dev。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_cline_usage, ClineUsage};
pub use detect::CLINE_FORMAT;
pub use versions::ui_messages_doc1::CLINE_PARSER_VERSION;
pub use versions::{
    ui_messages_doc1, CLINE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS,
};

/// VS Code 扩展 globalStorage 目录名（disk.ts 经宿主 globalStorageFsPath 定位任务目录）。
pub const CLINE_EXT_GLOBAL_STORAGE: &str = "saoudrizwan.claude-dev";

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// 剥 UTF-8 BOM（detect 指纹探测与整文件解析共用；无 BOM 输入原样返回）。
fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes)
}

/// Cline 适配器（无状态）。
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

/// 手工根兼容两种形状：globalStorage 目录（含 tasks/）或 tasks 目录本身。
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
        // VS Code 默认 globalStorage（Windows %APPDATA%/Code、unix XDG、macOS
        // ~/Library/Application Support/Code）；APPDATA/XDG_CONFIG_HOME 只用于
        // 解析平台默认位置，不是 Cline 自己的环境覆盖。
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
            // 手工根可能是 tasks/ 本身；默认根必须含 tasks/ 子目录。
            let Some(tasks) = cline_tasks_dir(&root) else {
                continue;
            };
            // tasks/<taskId>/ui_messages.json：深度 2，有界枚举。
            let files = crate::adapters::framework::enumerate_files_bounded(&tasks, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "ui_messages.json")
                    .unwrap_or(false)
            });
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
        // 唯一格式实现；ui_messages.json 无版本字段，detect 不按版本分派。
        // 注册表扩展多实现后在此按 detect 结论分派。
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
                    "文档级证据（固定源码 dcf8c3c），待真实样本：四互斥桶各自可选，total 派生".into(),
                ),
                "say(api_req_started|deleted_api_reqs|subagent_usage).text: tokensIn/tokensOut/cacheWrites/cacheReads",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "cacheReads reported（可选字段）",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "cacheWrites reported（可选字段）",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(
                    "文档级证据，待真实样本：api_req_started 逐请求（已合并 finished）；聚合记录按 1 条计不是底层请求数".into(),
                ),
                "只有三类 usage 载体 say 计数；compaction/无 usage 的 say 不算请求",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Unavailable(
                    "固定源码未证实逐请求模型字段；逐请求模型归属需核验（adapters.md A03）".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "消息 ts（epoch 毫秒）：api_req_started 为 source_start，聚合记录 uncertain",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(
                    "文档级证据，待真实样本：micro-USD 记账；来源口径（扩展自算价目 or 供应商账单）未证实，按估算入账".into(),
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
                    "${XDG_CONFIG_HOME:-~/.config}/Code/User/globalStorage/saoudrizwan.claude-dev (Linux)"
                ],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "tasks/<taskId>/ui_messages.json",
                "profile": "无 profile 概念（宿主 VS Code 按用户目录）；CLI SDK 单独探测，未接入",
            }),
            detection: serde_json::json!({
                "magic": "文件头 64 KiB 指纹：JSON 数组且含 say 消息（剥 UTF-8 BOM）",
                "version_field": "无版本字段；格式版本为文档级 ui-messages-doc-1（固定源码 dcf8c3c）",
                "fail_closed": true,
                "unknown_version": "未文档化 say 种类或非 say 记录类型：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "api_req_started（已与 api_req_finished 合并，final，task+say+ts 身份）",
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
                "cross_source": "CLI SDK / api_conversation_history.json 未接入，不与 ui_messages.json 相加",
            }),
            integrity: serde_json::json!({
                "success_only": "只统计带 usage 数字的载体；无 usage 的 api_req_started（未完成请求）无事件、部分可用，其余记录继续入账",
                "hidden_calls": "deleted/subagent 聚合是可见用量；标题生成等辅助调用无证据，未观测",
                "sampling": "上游 JSON parse 失败静默忽略；本适配器逐条记诊断后跳过",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（type/say/ts/text 内 tokensIn/tokensOut/cacheWrites/cacheReads/cost），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": CLINE_PARSER_VERSION,
                "format_evidence": "固定源码 dcf8c3c（getApiMetrics.ts/disk.ts，A03）；文档级证据，本机无真实样本（not_found）",
                "upgrade_policy": "say 种类/记录类型/字段形状偏离 fail closed；取得真实样本后扩展接受集与逐请求模型归属",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "无变化探测短路；有变化全量重读（32 MiB 有界）+ upsert 幂等",
                "pause_cancel": "文件间可停；单文件读取有界",
            }),
            limitations: vec![
                "全部字段口径为文档级证据（固定源码 dcf8c3c，A03），本机无真实样本（not_found）；首份真实 fixture 到达后逐字段核验".into(),
                "ui_messages.json 实际消息词汇（ask、say=text/tool 等非用量种类）未在固定源码中枚举：按未文档化处理 fail closed，待真实样本扩展接受集".into(),
                "deleted_api_reqs/subagent_usage 聚合无法分解底层请求数：call_count 按聚合记录计（下限），token 值为聚合真值".into(),
                "subagent_usage 聚合与其明细任务文件（若存在）无共享 ID 证据，跨文件不去重，真实样本需核验是否双源".into(),
                "cost 来源口径（扩展自算价目 or 供应商账单）未证实：按 micro-USD estimated 入账，不与远端账单相加".into(),
                "逐请求模型字段未证实：model 保持 unknown，不猜归属".into(),
                "消息删除后再次原样恢复的键已被墓碑排除，重新出现时保持排除（Corrected 优先）".into(),
                "整写 JSON 每次变化全量重读（32 MiB 有界），成本随文件大小线性；靠 upsert 幂等保证不双计".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}
