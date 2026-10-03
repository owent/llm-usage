//! Codex 适配器（独立目录合同 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测与版本分派；
//! - [`versions`]：已验证格式实现的注册与映射，未知版本默认回退最新内置解析器；
//! - 历史版本的格式实现一律保留在本目录内，不再回到根级单文件。
//!
//! 原始格式证据见各版本模块文件头；M2-A/M2-D 验证记录。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_codex, map_codex_record, CodexRecordUsage, CodexUsage};
pub use detect::CODEX_FORMAT;
pub use versions::{
    rollout_legacy, rollout_v1, LATEST_IMPL_ID, SUPPORTED_CLI_VERSIONS, VERIFIED_VERSION_IMPLS,
};

pub const CODEX_ENV_HOME: &str = "CODEX_HOME";

/// Codex 适配器（无状态）。
pub struct CodexAdapter;

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexAdapter {
    pub fn new() -> Self {
        CodexAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for CodexAdapter {
    fn adapter_id(&self) -> &'static str {
        "codex"
    }

    fn agent(&self) -> &'static str {
        "codex"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get(CODEX_ENV_HOME) {
            roots.push((
                std::path::PathBuf::from(home),
                RootBasis::EnvOverride(CODEX_ENV_HOME.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".codex"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            // Archive/unarchive moves the same rollout. Keep the root instance
            // and native response/session keys, even when both copies exist.
            let files = ["sessions", "archived_sessions"]
                .into_iter()
                .flat_map(|dir| {
                    crate::adapters::framework::enumerate_files_bounded(&root.join(dir), 3, &|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
                    })
                })
                .collect::<Vec<_>>();
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "codex@{}",
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
        // 版本注册表分派（探测/扫描同一注册表）：0.153+ → rollout_v1，
        // 0.139–0.151 旧载体 → rollout_legacy，未收录 → LatestFallback（rollout_v1）。
        versions::dispatch_scan(target, stored, limits, now_ms)
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
        fields.insert("tokens".into(), field(Availability::Available, "token_usage_record 逐次六字段；cached⊆input、reasoning⊆output、total=input+output（本机 319/319 实读成立）"));
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cached_input_tokens reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(
                    "真实样本仅覆盖 cache_write=0；cache_write⊆input 为映射假设，矛盾进诊断".into(),
                ),
                "cache_write_input_tokens reported",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "每条 token_usage_record 是一次模型调用；response_id 稳定身份",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(
                    "usage 无 model 字段；按不晚于调用的 turn_context 位置归属，无证据 unknown"
                        .into(),
                ),
                "turn_context.model",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "envelope ISO8601 毫秒 UTC，source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("本地无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert("latency".into(), field(Availability::Partial("task_complete 有 turn 级 duration_ms/time_to_first_token_ms；M2-A 未映射到逐次事件（迟到信息会引发同键冲突），记为缺口".into()), "turn 级"));
        CapabilityTable {
            adapter_id: "codex".to_string(),
            product: "Codex CLI / 桌面 / IDE 宿主".to_string(),
            surfaces: vec!["cli".into(), "vscode-extension".into(), "desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["$CODEX_HOME", "<home>/.codex"],
                "env_override": CODEX_ENV_HOME,
                "manual_roots": true,
                "bounded": true,
                "pattern": "sessions/<YYYY>/<MM>/<DD>/rollout-*.jsonl + archived_sessions/rollout-*.jsonl",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=session_meta",
                "version_field": "payload.cli_version",
                "registry": "adapters/codex/versions 注册表分派",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本先尝试最新内置解析器（latest_fallback），通过校验的数据带兼容标记统计；有证据不兼容才拒绝",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "token_usage_record（final，response_id 身份）；0.139–0.151 无该载体，按 token_count.last_token_usage 的 total 增量判据发逐次事件（rollout_legacy，身份 seq:{session}:{行号}）",
                "cumulative_snapshot": "token_count.total_token_usage 取最终值；0.153+ compaction 携带记录与 0.139–0.151 压缩摘要回声（delta==0 且 last 变化）均排除出快照；Σ逐次==最终快照+Σ携带（实读核对）",
                "last_token_usage": "0.153+ 为逐次回声忽略防双计；0.139–0.151 为唯一逐次载体，按增量判据去重（delta==0 且 last 未变=重复上报）",
                "aborted": "turn_aborted 已观测；已返回部分按 token_count 回声记账",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "session_meta.parent_thread_id 存在 ⇒ sub_agent；子 Agent 是独立 rollout 文件",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
            }),
            dedup: serde_json::json!({
                "primary": "resp:{response_id}（实例命名空间）",
                "fallback": "seq:{session UUID}:{行号}（缺 response_id，已验证替代）",
                "cross_source": "state_5.sqlite threads.tokens_used 是另一存储的线程级累计；M2-A 只读 rollout，不相加",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "aborted_turns": "usage 按已返回部分记账",
                "hidden_calls": "未知；auto-review 等子 Agent 会话是独立 rollout 文件，各自计入",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::rollout_v1::CODEX_PARSER_VERSION,
                "format_evidence": "M0 本机 fixture（0.155.0-alpha.16.3，3 会话）；M2-D 逐版本 fixture（0.153/0.154）；2026-09-26 本机 238 个 0.139–0.151 文件全量实读取证（rollout_legacy）",
                "upgrade_policy": "未收录版本 latest_fallback 兼容尝试；逐版本 fixture 核验后升为已验证",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "turn 级 duration/TTFT 未映射到逐次事件（迟到信息同键冲突风险）".into(),
                "cache_write>0 仅有合成样本；cache_write⊆input 为映射假设".into(),
                "附属 SQLite（state_5 等）不在 M2-A 范围；threads.tokens_used 不与 rollout 相加"
                    .into(),
                "符号链接/ junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
                "无 response_id 记录用会话 UUID+行号身份，文件同位替换后可能形成新键".into(),
                "解析器规则升级后自动重扫已消费游标；未知格式仍保留兼容状态与诊断".into(),
            ],
        }
    }
}
