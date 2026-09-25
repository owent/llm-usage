//! Qwen Code 适配器（独立目录合同 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测与版本分派；
//! - [`versions`]：已验证格式实现的注册与映射（统一结构；格式锚点是固定源码
//!   commit，不做 CLI 版本白名单）；
//! - 历史版本的格式实现一律保留在本目录内，不再回到根级单文件。
//!
//! 原始格式证据见各版本模块文件头（A18）；本目录化迁移自根级单文件 qwen.rs
//! 平移（M2 目录化迁移，V30），行为合同不变。

pub mod detect;
pub mod versions;

pub use detect::QWEN_FORMAT;
pub use versions::chatrecord_085e98c0::QWEN_PARSER_VERSION;
pub use versions::{
    chatrecord_085e98c0, LATEST_IMPL_ID, QWEN_FORMAT_VERSION, VERIFIED_VERSION_IMPLS,
};

/// Qwen Code 适配器（无状态）。
pub struct QwenAdapter;

impl Default for QwenAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl QwenAdapter {
    pub fn new() -> Self {
        QwenAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for QwenAdapter {
    fn adapter_id(&self) -> &'static str {
        "qwen"
    }

    fn agent(&self) -> &'static str {
        "qwen-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".qwen"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let tmp = root.join("tmp");
            if !tmp.is_dir() {
                continue;
            }
            // tmp/<project_id>/chats/<sessionId>.jsonl：深度 2，有界枚举。
            let files = crate::adapters::framework::enumerate_files_bounded(&tmp, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".jsonl"))
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
            "qwen@{}",
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
        // 当前所有已验证版本共用 chatrecord_085e98c0；注册表扩展多实现后在此按选择分派。
        versions::chatrecord_085e98c0::scan(target, stored, limits, now_ms)
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
                    "六分类各自可选；thoughts/tool 与 input/output 包含关系未核验，total 只取直报不派生".into(),
                ),
                "usageMetadata: prompt/candidates/cached/thoughts/toolUsePrompt/total（固定源码）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Available,
                "cachedContentTokenCount reported（可选字段）",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Unavailable("格式内无缓存创建字段".into()),
                "无",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "recordAssistantTurn 每次模型调用一条 assistant 记录（uuid 身份）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "记录自带 model 字段（request_field）；缺失 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "记录 ISO8601 timestamp，source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("本地无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("ChatRecord 无逐次延迟字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "qwen".to_string(),
            product: "Qwen Code CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["<home>/.qwen"],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "tmp/<project_id>/chats/<sessionId>.jsonl",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type ∈ ChatRecord 四值且 uuid/sessionId/timestamp 必填齐全",
                "version_field": "record.version 逐条存 schema_version（不做白名单）；格式版本=固定源码 commit",
                "registry": "adapters/qwen/versions 注册表统一结构（唯一条目=固定源码 commit 锚点，非版本白名单）",
                "fail_closed": true,
                "unknown_version": "type/subtype 超出固定源码枚举或载体外 usageMetadata：整文件拒绝",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "assistant 记录 usageMetadata（final，uuid 身份）",
                "goal_meter": "goal_state/goal_runtime/goal_turn_end 是 Goal 控制记录；goal.tokensUsed 为跨 turn 累计表，明确忽略（计入即双计）",
                "goal_context_calls": "goalContext 标注的 assistant 记录仍是真实模型调用，照常计入",
                "chat_compression": "压缩检查点控制记录，明确忽略",
                "session_model": "daemon 恢复绑定记录，不归给历史调用（不产事件）",
                "aborted": "格式内无证据；未观测",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "isSidechain=true 或 agentId 存在 ⇒ sub_agent；与主会话同文件，无双计",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
            }),
            dedup: serde_json::json!({
                "primary": "qwen:{uuid}（实例命名空间）",
                "fallback": "seq:{sessionId}:{行号}（缺 uuid，记诊断）",
                "cross_source": "只读 chats JSONL；无其他本地用量存储接入",
            }),
            integrity: serde_json::json!({
                "success_only": "格式内无失败调用证据；只统计带 usageMetadata 的 assistant 记录",
                "hidden_calls": "未观测；子 Agent 记录与主会话同文件按同口径计入",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（uuid/sessionId/timestamp/type/subtype/version/model/agentId/isSidechain/goalContext/usageMetadata），prompt 日志侧写禁用，正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::chatrecord_085e98c0::QWEN_PARSER_VERSION,
                "format_evidence": "qwen-code 源码固定 commit 085e98c00cac2f8dd29eb39c760409bc6da889a9（ChatRecord/recordAssistantTurn/Goal 协议）",
                "upgrade_policy": "schema 以固定 commit 为准；type/subtype 偏离 fail closed，重固定新 commit 后扩展枚举",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "usageMetadata 六分类的包含关系（cached/thoughts/tool 是否子集）逐 provider 未核验：total 只取直报，thoughts/tool 不并入任何字段".into(),
                "record.version 逐条存 schema_version 但不做版本白名单（格式锚点是固定源码 commit）".into(),
                "缺 uuid 记录用 sessionId+行号身份，文件同位替换后可能形成新键".into(),
                "本机无真实样本（not_found）；合同测试基于按固定源码构造的合成样本".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}
