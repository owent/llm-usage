//! Claude Code 适配器（独立目录约定 architecture.md#adapter-layout，V30 目录迁移）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：格式探测（首行记录类型集合）；
//! - [`versions`]：旧文档锚点与逐条 2.1.197 原生版本独立绑定；
//! - 历史版本的格式实现一律保留在本目录内，不再回到根级单文件。
//!
//! 原始格式依据见各版本模块文件头；目录迁移不改已验收的拒绝语义
//! （未文档化记录 type / 载体外 usage 字段 ⇒ 整文件 fail closed，V17）。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_claude_transcript, ClaudeTranscriptUsage};
pub use detect::CLAUDE_FORMAT;
pub use versions::transcript_doc1::CLAUDE_PARSER_VERSION;
pub use versions::{
    transcript_doc1, CLAUDE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS,
};

pub const CLAUDE_ENV_HOME: &str = "CLAUDE_CONFIG_DIR";

/// Claude Code 适配器（无状态）。
pub struct ClaudeAdapter;

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        ClaudeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for ClaudeAdapter {
    fn adapter_id(&self) -> &'static str {
        "claude"
    }

    fn agent(&self) -> &'static str {
        "claude-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get(CLAUDE_ENV_HOME) {
            roots.push((
                std::path::PathBuf::from(home),
                RootBasis::EnvOverride(CLAUDE_ENV_HOME.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".claude"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let projects = root.join("projects");
            if !projects.is_dir() {
                continue;
            }
            // projects/<project>/<session>.jsonl 深度 2；subagents/ 内 transcript 深度 3。
            let files = crate::adapters::framework::enumerate_files_bounded(&projects, 3, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".jsonl") || n.contains(".jsonl.superseded-"))
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
            "claude@{}",
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
        // 当前所有已验证格式共用 transcript_doc1；注册表扩展多实现后在此按选择分派。
        versions::transcript_doc1::scan(target, stored, limits, now_ms)
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
                Availability::Partial("2.1.197 原生实样：正桶报告值；默认零与完整总量未知".into()),
                "message.usage 四字段 input/output/cache_read/cache_creation（全必填）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("2.1.197 已核对；其他版本/协议未核验".into()),
                "原生正 cache_read_input_tokens 为 reported；零未知，本次无正缓存实样",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial("2.1.197 已核对；其他版本/协议未核验".into()),
                "原生正 cache_creation_input_tokens 为 reported；零未知，本次无正缓存实样",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "2.1.197 两次 HTTP 请求生成四条内容块；按 requestId 或 message.id 去重计两次调用",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial("2.1.197 已核对；其他版本/协议未核验".into()),
                "message.model 记录自带字段（request_field）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial("2.1.197 已核对；其他版本/协议未核验".into()),
                "条目 ISO8601 时间戳，source_completion 口径",
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
                Availability::Unavailable(
                    "transcript 条目无逐次延迟字段（OTel api_response 有 duration_ms，未接入）"
                        .into(),
                ),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "claude".to_string(),
            product: "Claude Code CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["$CLAUDE_CONFIG_DIR", "<home>/.claude"],
                "env_override": CLAUDE_ENV_HOME,
                "manual_roots": true,
                "bounded": true,
                "pattern": "projects/<project>/*.jsonl（含 orphaned/superseded 变体）与 projects/<project>/<session>/subagents/*.jsonl",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type ∈ {user, assistant, system}",
                "version_field": "assistant.version 逐条绑定；2.1.197 原生已核对；无版本旧文档锚点独立保留",
                "fail_closed": true,
                "unknown_version": "未文档化记录 type 或载体外 usage 字段：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "assistant 条目 message.usage（final，requestId 或 message.id 身份）",
                "one_response_many_entries": "一个 API 响应按内容块持久化为多条目（官方文档）；usage 重复，按 requestId upsert 去重",
                "cumulative_snapshot": "transcript 无累计快照；goal/账单侧写不接入",
                "aborted": "格式内无证据；未观测",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "isSidechain=true 或 subagents/ 路径 ⇒ sub_agent；子 Agent transcript 是独立文件，跨文件同 requestId 靠 upsert 防双计",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
            }),
            dedup: serde_json::json!({
                "primary": "req:{requestId}（实例命名空间）",
                "fallback": "msg:{message.id} → uuid:{条目 uuid} → seq:{sessionId}:{行号}（逐级记诊断）",
                "orphaned_superseded": "旧 transcript 变体与现行文件内容重叠；稳定身份 upsert 防双计",
                "cross_source": "OTel query_source 汇总（main/subagent/auxiliary）未接入，不与 transcript 相加",
            }),
            integrity: serde_json::json!({
                "success_only": "格式内无失败调用证据；只统计已持久化的 assistant usage 条目",
                "hidden_calls": "auxiliary（compact 等）请求若写入 transcript 则按同口径计入；OTel 侧未接入",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "source_retention": "cleanupPeriodDays 清扫（默认 30 天）；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（type/timestamp/version/sessionId/requestId/uuid/isSidechain/message.{id,model,usage}），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::transcript_doc1::NATIVE_PARSER_VERSION,
                "format_evidence": "A01 文档与 2.1.197 原生 GLM 两模型；API/CLI/原生日志的正输入/输出独立相符",
                "upgrade_policy": "transcript 条目格式官方明示不稳定；偏离（未知 type、载体外 usage）fail closed，取得真实样本后扩展",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "2.1.197 智谱兼容端点主循环实样已核对；Anthropic 原生、重试、辅助及子 Agent 尚未实测".into(),
                "transcript 条目格式官方明示非稳定合同，任何偏离 fail closed 而非猜测".into(),
                "原生版本四桶仍校验完整形状；默认零未知，缺桶不补零；本地没有渠道字段，provider 未知".into(),
                "OTel telemetry（query_source 分类、duration_ms）不接入，不与 transcript 相加".into(),
                "无 requestId/message.id/uuid 记录用 sessionId+行号身份，文件同位替换后可能形成新键".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }

    fn should_scan_unchanged(&self, stored: &crate::adapters::framework::StoredScanState) -> bool {
        stored
            .parse_context
            .as_ref()
            .and_then(|c| c.get("native_rules_version"))
            .and_then(serde_json::Value::as_u64)
            != Some(1)
    }
}
