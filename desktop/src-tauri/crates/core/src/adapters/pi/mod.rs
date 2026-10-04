//! pi（pi-coding-agent）适配器（独立目录约定 architecture.md#adapter-layout，
//! V30 自根级 adapters/pi.rs 目录化迁移）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测与版本分派；
//! - [`versions`]：已验证格式实现的注册与映射，未收录数值默认回退最新内置解析器，
//!   固定源码已确认不兼容的版本（v1/v2/缺失 version）才 fail closed；
//! - 历史版本的格式实现一律保留在本目录内，不再回到根级单文件。
//!
//! 原始格式依据见 [`versions::session_v3`] 文件头（固定源码 pi-mono b4559750 +
//! 本机 fixture）；pi/omp 家族共享的 usage 解析与事件构造随扫描实现放在
//! [`versions::session_v3`] 并在此再导出（`map_pi_family` 仍留在跨 Agent 的
//! 根级 usage_map.rs，勿动）。

pub mod detect;
pub mod versions;

pub use detect::PI_FORMAT;
pub use versions::session_v3::PI_PARSER_VERSION;
pub use versions::{LATEST_IMPL_ID, SUPPORTED_SESSION_VERSION, VERIFIED_VERSION_IMPLS};

// pi/omp 家族共享辅助（原根级 pi.rs 的 pub(crate) 项，保持 crate 内旧路径可用；
// map_cost 仅本目录内使用，不再对外再导出）。
pub(crate) use versions::session_v3::{
    build_pi_family_event, diag, family_entry_key, json_str, parse_entry_ts, parse_usage,
    UsageEventBase,
};

pub const PI_ENV_AGENT_DIR: &str = "PI_CODING_AGENT_DIR";
pub const PI_ENV_SESSION_DIR: &str = "PI_CODING_AGENT_SESSION_DIR";

/// pi 适配器（无状态）。
pub struct PiAdapter;

impl Default for PiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PiAdapter {
    pub fn new() -> Self {
        PiAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for PiAdapter {
    fn adapter_id(&self) -> &'static str {
        "pi"
    }

    fn agent(&self) -> &'static str {
        "pi"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // (sessions 目录, basis)；root 统一取 sessions 目录，跨来源去重后同目录只扫一次。
        let mut candidates: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx.env.get(PI_ENV_AGENT_DIR) {
            candidates.push((
                std::path::PathBuf::from(dir).join("sessions"),
                RootBasis::EnvOverride(PI_ENV_AGENT_DIR.to_string()),
            ));
        }
        if let Some(dir) = ctx.env.get(PI_ENV_SESSION_DIR) {
            candidates.push((
                std::path::PathBuf::from(dir),
                RootBasis::EnvOverride(PI_ENV_SESSION_DIR.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            candidates.push((
                home.join(".pi").join("agent").join("sessions"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            // 手工根语义：含 sessions 子目录按 agent 根解析，否则按 sessions 目录本身。
            let sessions = if manual.join("sessions").is_dir() {
                manual.join("sessions")
            } else {
                manual.clone()
            };
            candidates.push((sessions, RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (sessions, basis) in candidates {
            if !sessions.is_dir() {
                continue;
            }
            // sessions/<encoded-cwd>/*.jsonl：深度 2，有界枚举；根下散落 jsonl 一并接受。
            let files = crate::adapters::framework::enumerate_files_bounded(&sessions, 2, &|p| {
                p.extension().and_then(|e| e.to_str()) == Some("jsonl")
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot {
                    root: sessions,
                    basis,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "pi@{}",
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
        // 当前所有可尝试版本共用 session_v3（session 头内部按注册表分派，
        // 已确认不兼容的版本跳过并记诊断）；注册表扩展多实现后在此按选择分派。
        versions::session_v3::scan(target, stored, limits, now_ms)
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
                Availability::Available,
                "Usage 六字段；input/cacheRead/cacheWrite 互斥（固定源码两个 provider 实现均如此规范化），totalTokens=四桶之和；reasoning⊆output 不再加",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cacheRead reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Available,
                "cacheWrite reported；cacheWrite1h 是 cacheWrite 子集，不再加",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "assistant message 每条一次调用；独立 usage/compaction/branch_summary/toolResult usage 各计一次辅助调用",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "assistant/provider、model 为请求自身字段；独立 usage 同；compaction/branch_summary 无模型字段，按不晚于它的 model_change 归属，无证据 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "条目 timestamp（ISO8601 UTC，追加即完成时）source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(
                    "usage.cost 为 Agent 自带价目估算（非供应商账单）；价目版本不随文件记录，>0 才映射为 estimated，0 与无价目不可区分记 unknown".into(),
                ),
                "usage.cost.total（USD）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("session JSONL 无逐次延迟/TTFT 字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "pi".to_string(),
            product: "pi（pi-coding-agent）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.pi/agent/sessions"],
                "env_override": [PI_ENV_AGENT_DIR, PI_ENV_SESSION_DIR],
                "manual_roots": "含 sessions 子目录按 agent 根解析，否则按 sessions 目录本身",
                "bounded": true,
                "pattern": "sessions/<encoded-cwd>/*.jsonl（散落根下 jsonl 一并接受）",
                "profile": "无 profile 概念（config.ts 未定义）",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=session",
                "version_field": "version（固定源码 CURRENT_SESSION_VERSION=3）",
                "registry": "adapters/pi/versions 注册表分派",
                "fail_closed": true,
                "unknown_version": "v1/v2 及缺失 version 有固定源码证据不兼容（unsupported_version 拒绝）；其余未收录数值（如 4）先尝试最新内置解析器（latest_fallback），通过校验的数据带兼容标记统计",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "message(role=assistant).usage（final；responseId 可选作 origin_call_id）",
                "auxiliary": "独立 usage 条目（kind 如 cache_warm）、compaction.usage、branch_summary.usage、toolResult.usage 各计一次辅助调用",
                "error_aborted": "stopReason=error/aborted → error_status；无 usage 的 assistant 计调用、token 未知不补零",
                "streaming": "流式部分值不落盘（固定源码：完成条目才追加），文件内只有 final",
                "compaction": "tokensBefore 是上下文估算，不是 usage，不计账",
                "branch": "文件内全条目求和（含放弃分支，调用均已计费；与固定源码 getSessionStats 同口径）",
                "fork": "fork 逐字复制条目（id/parentId/timestamp 不变）到新文件，继承不是新调用；事件键四元组实例内幂等去重",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）", "原子整写"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            dedup: serde_json::json!({
                "primary": "pi:{message|usage|compaction|branch_summary|toolresult}:{entry id}:{parentId}:{timestamp}（实例命名空间）",
                "fork_copies": "fork 复制件四元组逐字相同：内容逐字相同者 upsert 幂等 Keep；但复制条目在 fork 文件中会带 fork 会话身份（session_id/parent_session_id 与本文件头一致），与源文件已存事件同键不同内容，仲裁为 conflict 并保留先扫者——净效果不双计",
                "cross_source": "无第二本机来源；auth/models-store 非用量不读",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "cache_warm 等辅助调用有独立 usage 条目即覆盖；无条目的辅助调用（如未启用/旧版）不可见",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "no_timestamp": "条目必有 timestamp；缺失跳过并记诊断",
            }),
            maintenance: serde_json::json!({
                "parser_version": PI_PARSER_VERSION,
                "format_evidence": "固定源码 pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e（types.ts/session-manager.ts/config.ts/provider 实现）；本机 0.87.1 sessions 为空（no_data），fixture 全部合成",
                "upgrade_policy": "未收录 session version 先 latest_fallback 兼容尝试；v1/v2 及缺失 version 有固定源码证据不兼容不尝试；逐版本 fixture 核验后升为已验证",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界；不启动 Agent",
            }),
            limitations: vec![
                "本机无真实会话（0.87.1 sessions 空）：全部 fixture 合成，真实核对状态 no_data".into(),
                "responseModel 与 model 不一致时以请求字段 model 为准，差异不另记".into(),
                "usage.cost 为 Agent 估算；0 与无价目不可区分，均不映射".into(),
                "fork 继承条目归属到字典序首个被扫文件的会话（逐字相同，谁先谁留），distinct 会话数不因此虚增".into(),
                "符号链接/junction 不跟随；Windows 身份靠创建时间+首采样".into(),
                "cache_warm 之外的独立 usage kind 未见真实样本（固定源码仅 cache-warmer 一处写入）".into(),
                "latest_fallback 文件的解析器升级后不自动重扫已消费游标；显式重扫可重新尝试".into(),
            ],
        }
    }
}
