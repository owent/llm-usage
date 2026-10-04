//! oh-my-pi（omp）适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测与版本分派（首行 title/session 闸口四态语义不变）；
//! - [`versions`]：已验证格式实现的注册与映射，未知版本默认回退最新内置解析器；
//! - 历史版本的格式实现一律保留在本目录内，不再回到根级单文件。
//!
//! 原始格式依据见各版本模块文件头；M2-B/C 验证记录。V30 目录迁移自根级
//! adapters/omp.rs，行为除未知版本策略外不变，不重建来源、不重置游标。

pub mod detect;
pub mod versions;

pub use detect::OMP_FORMAT;
pub use versions::{session_v3, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

pub const OMP_ENV_AGENT_DIR: &str = "PI_CODING_AGENT_DIR";
pub const OMP_ENV_SESSION_DIR: &str = "PI_CODING_AGENT_SESSION_DIR";

/// oh-my-pi 适配器（无状态）。
pub struct OmpAdapter;

impl Default for OmpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OmpAdapter {
    pub fn new() -> Self {
        OmpAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for OmpAdapter {
    fn adapter_id(&self) -> &'static str {
        "omp"
    }

    fn agent(&self) -> &'static str {
        "oh-my-pi"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // (sessions 目录, basis)；root 统一取 sessions 目录，跨来源去重后同目录只扫一次。
        let mut candidates: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx.env.get(OMP_ENV_AGENT_DIR) {
            candidates.push((
                std::path::PathBuf::from(dir).join("sessions"),
                RootBasis::EnvOverride(OMP_ENV_AGENT_DIR.to_string()),
            ));
        }
        if let Some(dir) = ctx.env.get(OMP_ENV_SESSION_DIR) {
            candidates.push((
                std::path::PathBuf::from(dir),
                RootBasis::EnvOverride(OMP_ENV_SESSION_DIR.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            candidates.push((
                home.join(".omp").join("agent").join("sessions"),
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
            // sessions/<cwd>/*.jsonl 主会话（深度 2）；子 Agent <cwd>/<ts>_<uuid>/*.jsonl
            // （深度 3）与嵌套子 Agent（深度 4）；伴生 .json/.md/.log 不接受。
            let files = crate::adapters::framework::enumerate_files_bounded(&sessions, 4, &|p| {
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
            "omp@{}",
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
        // 当前所有已验证版本共用 session_v3；注册表扩展多实现后在此按选择分派。
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
                "Usage 六字段实读核验：input/cacheRead/cacheWrite 互斥，totalTokens=四桶之和（8752 条全成立）；reasoningTokens⊆output 可选（85 条）不再加",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cacheRead reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Available, "cacheWrite reported"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "assistant message 每条一次调用；独立 usage/compaction/branch_summary/toolResult usage 各计一次辅助调用（本机后四者均未携带 usage）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "assistant 自带 provider/model（8752 条全在场）；compaction/branch_summary 按不晚于它的 model_change 归属（omp 落盘为 model=\"provider/model\" 组合字段）",
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
                    "usage.cost 为 Agent 自带价目估算（非供应商账单）；本机 2 个会话观测到 cost.total>0；价目版本不随文件记录，>0 才映射为 estimated，0 与无价目不可区分记 unknown".into(),
                ),
                "usage.cost.total（USD）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Available,
                "assistant 自带 duration/ttft 浮点毫秒（omp 特有），入库四舍五入到 i64 毫秒",
            ),
        );
        CapabilityTable {
            adapter_id: "omp".to_string(),
            product: "oh-my-pi（omp，pi-coding-agent 分支）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.omp/agent/sessions"],
                "env_override": [OMP_ENV_AGENT_DIR, OMP_ENV_SESSION_DIR],
                "manual_roots": "含 sessions 子目录按 agent 根解析，否则按 sessions 目录本身",
                "bounded": true,
                "pattern": "sessions/<encoded-cwd>/*.jsonl 主会话；<cwd>/<ts>_<父uuid>/*.jsonl 子 Agent（嵌套再深一层）；伴生 .json/.md/.log 不读",
                "profile": "OMP_PROFILE 存在（omp.exe 字符串证据）但目录规则未核验，仅支持默认 ~/.omp",
            }),
            detection: serde_json::json!({
                "magic": "首行 title（v=1）或 session 记录；前 4 行内定位 session 头",
                "version_field": "session.version（本机 58/58 全为 3）",
                "registry": "adapters/omp/versions 注册表分派",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本先尝试最新内置解析器（latest_fallback），通过校验的数据带兼容标记统计；omp 旧版落盘格式未取证（无证据不兼容），不直接拒绝（与 pi 的 evidenced-incompatible 分支不同）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "message(role=assistant).usage（final；responseId 作 origin_call_id）",
                "auxiliary": "独立 usage 条目、compaction.usage、branch_summary.usage、toolResult.usage 各计一次辅助调用（本机均未携带 usage，实读核验）",
                "error_aborted": "stopReason=error/aborted → error_status；本机 147 条 error/aborted 全带 usage；无 usage 的 assistant 计调用、token 未知不补零",
                "streaming": "流式部分值不落盘（同 pi 固定源码口径），文件内只有 final",
                "compaction": "tokensBefore/tokensAfter 是上下文估算，不是 usage，不计账",
                "subagent": "子 Agent 文件在 <ts>_<父uuid>/ 目录内，自有 session 头；父子关联来自目录名（58 文件实读核验）",
                "fork": "fork 逐字复制条目（id/parentId/timestamp 不变）到新文件，继承不是新调用；事件键四元组实例内幂等去重（本机未观测到 fork）",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）", "原子整写"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            dedup: serde_json::json!({
                "primary": "omp:{message|usage|compaction|branch_summary|toolresult}:{entry id}:{parentId}:{timestamp}（实例命名空间）",
                "fork_copies": "fork 复制件四元组逐字相同：内容逐字相同者 upsert 幂等 Keep；但复制条目在 fork 文件中会带 fork 会话身份（session_id/parent_session_id 与本文件头一致），与源文件已存事件同键不同内容，仲裁为 conflict 并保留先扫者——净效果不双计",
                "cross_source": "~/.omp/logs 仅上下文估算 debug 行，无逐次用量，不存在与会话记录的重叠；agent.db/history.db 未取证不读",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "title-generator 等辅助调用在本机会话/日志中未观测到用量记录，不可见",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "no_timestamp": "条目必有 timestamp；缺失跳过并记诊断",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::session_v3::OMP_PARSER_VERSION,
                "format_evidence": "本机 18.2.7（scoop）58 会话逐类型实读核验 + 固定源码 oh-my-pi 62bc57b（Usage 口径）+ omp.exe 字符串（env 覆盖/OMP_PROFILE）",
                "upgrade_policy": "未收录版本 latest_fallback 兼容尝试；逐版本真实 fixture 核验后升为已验证",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界；不启动 Agent",
            }),
            limitations: vec![
                "OMP_PROFILE 命名 profile 的目录规则未核验，仅支持默认 ~/.omp".into(),
                "嵌套子 Agent（<Name>/<Name>.<sub>.jsonl）父会话取最近的 <ts>_<uuid> 祖先目录，隔代不归名".into(),
                "fork 继承条目归属到字典序首个被扫文件的会话（逐字相同，谁先谁留），distinct 会话数不因此虚增".into(),
                "usage.cost 为 Agent 估算；0 与无价目不可区分，均不映射".into(),
                "符号链接/junction 不跟随；Windows 身份靠创建时间+首采样".into(),
                "独立 usage 条目、fork、带 usage 的 compaction/branch_summary 本机未观测，实现按 pi 同口径，待真实样本".into(),
                "detect 接受 session 头在前的文件（与 pi 同形）；默认根 ~/.omp 与 ~/.pi 不重叠，把同一目录同时手工配给两个适配器会双计（用户配置责任）".into(),
                "latest_fallback 文件的解析器升级后不自动重扫已消费游标；显式重扫可重新尝试".into(),
            ],
        }
    }
}
