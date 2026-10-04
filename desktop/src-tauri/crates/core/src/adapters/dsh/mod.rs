//! DeepSeek Harness（DSH）适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：持久会话日志事件流的文档级指纹（无版本字段，不做版本分派）；
//! - [`versions`]：统一形状的格式注册表（唯一条目：文档级 session-log-doc-1）；
//! - 产品特有映射在 [`common`]（tokenUsage 四可选字段）。
//!
//! 原始格式依据（固定 token-meter README 46a7f68b0922371ce7144b668b90e377d8e799f4，
//! A08，按文档或源码实现，待真实样本核验；本机 2026-09-25 盘点 not_found）：
//! - `tokenUsage` 折叠持久日志的 `uncachedInputTokens`/`outputTokens`/
//!   `cacheReadTokens`/`cacheWriteTokens`；
//! - "A final assistant-message sample replaces streaming usage from the same
//!   attempt; `llm/retry-started` ends that replacement scope, so a retry in
//!   the same step contributes another billed attempt"；
//! - "Usage folds replace samples within each attempt; totals need not be
//!   monotone"（累计值可下修）；
//! - `contextPressure`（pressureTokens/projectedTokens/contextWindow）与
//!   `contextBreakdown`（systemTokens/toolsTokens/messageTokens）是估算/
//!   组合视图，"not its provider-billed size"，不进入用量；
//! - 事件词汇（README 枚举）：`step/start`、`assistant/message`、
//!   `llm/retry-started`、`request/context`、`request/header`、`image/offload`。
//!   持久日志的落盘路径与行序列化未在 pinned README 记载：发现只接受手工根，
//!   JSONL 行形状为合成假设（fixtures 标 synthetic，待真实样本核验）。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_dsh_usage, DshUsage};
pub use detect::DSH_FORMAT;
pub use versions::session_log_doc1::DSH_PARSER_VERSION;
pub use versions::{session_log_doc1, DSH_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// DSH 适配器（无状态）。
pub struct DshAdapter;

impl Default for DshAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DshAdapter {
    pub fn new() -> Self {
        DshAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for DshAdapter {
    fn adapter_id(&self) -> &'static str {
        "dsh"
    }

    fn agent(&self) -> &'static str {
        "deepseek-harness"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // pinned README 未记载持久日志的默认落盘路径与环境覆盖：只接受手工根
        // （真实安装路径取得样本后补充默认候选）。
        let mut out = Vec::new();
        for manual in &ctx.manual_roots {
            // 手工根 = 会话日志目录（根下 *.jsonl，深度 1，有界枚举）。
            let files = crate::adapters::framework::enumerate_files_bounded(manual, 1, &|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e == "jsonl")
                    .unwrap_or(false)
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot {
                    root: manual.clone(),
                    basis: RootBasis::Manual,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "dsh@{}",
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
        // 唯一格式实现；无版本字段，detect 不按版本分派。
        versions::session_log_doc1::scan(target, stored, limits, now_ms)
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
                    "文档级证据（固定 token-meter README 46a7f68），待真实样本：tokenUsage 四可选字段".into(),
                ),
                "assistant/message usage: uncachedInputTokens/outputTokens/cacheReadTokens/cacheWriteTokens",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "cacheReadTokens reported（attempt 可选值）",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "cacheWriteTokens reported（attempt 可选值）",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "attempt = step 内至 retry 边界的一次计费尝试（llm/retry-started 新开替换范围）；同 attempt 流式样本被 final 替换",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Unavailable("pinned README 未证实日志内逐请求 model/provider 字段（route 经 llm 服务运行时解析）".into()),
                "无",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Unavailable(
                    "pinned README 未记载逐事件时间字段：occurred_at 用观察时间（observed_at 口径），日归属受此限制".into(),
                ),
                "无源时间戳证据；待真实样本核验",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("持久日志无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("持久日志无逐次延迟/TTFT 字段证据".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "dsh".to_string(),
            product: "DeepSeek Harness (DSH)".to_string(),
            surfaces: vec!["harness".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "<manual_root>/*.jsonl（深度 1）",
                "profile": "无 profile 概念证据；默认落盘路径未文档化，待真实样本补充",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type ∈ README 枚举六事件",
                "version_field": "无版本字段；格式版本为文档级 session-log-doc-1（固定 README 46a7f68）",
                "fail_closed": true,
                "unknown_version": "未文档化事件 type：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "streaming_to_final": "同 attempt 的后续 usage 样本按更高 source_revision 替换（Replace 撤销旧贡献）；attempt 边界（retry/step）到达时以 Corrected 语义之上的 Final 样本收口",
                "retry": "llm/retry-started 结束替换范围并新开 attempt（新键）：同 step 的 retry 各自计费",
                "totals_not_monotone": "final 可小于流式值（下修），不做钳制",
                "open_attempt": "日志尾部未闭合的 attempt 以 Partial 入账（值可见）；下轮边界事件收口为 Final",
                "aggregates": "deriveTurnTokenUsage 的整 turn 聚合不落盘，不双计（逐 attempt 已覆盖）",
                "estimates": "contextPressure（pressureTokens/projectedTokens/contextWindow）与 contextBreakdown 是估算/组合视图，不进入用量",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 折叠状态（step/attempt/序号/待收口样本）",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；预算停在完整行边界",
                "half_line": "半行不前移游标",
                "rescan": "折叠计数器重置从头重折（确定性重放）；revision_floor 与 attempt 键集跨重扫保留，防同级内容冲突并墓碑消失 attempt",
            }),
            dedup: serde_json::json!({
                "primary": "{file_identity}:s{step}:a{attempt}（attempt 键；重放确定性）",
                "revision": "样本序号 + 跨轮单调 revision_floor：重放/重扫用更高修订号替换，不产生同级冲突",
                "vanished_attempts": "重扫后消失的 attempt 键墓碑（Corrected/Excluded），防截断改写双计",
                "cross_source": "session projections（tokenUsage/contextPressure/contextBreakdown）是同一日志的派生视图，不与事件相加",
            }),
            integrity: serde_json::json!({
                "success_only": "usage 样本来自成功 assistant/message 的 provider 上报；无 usage 的消息无事件（部分可用）",
                "hidden_calls": "attempt 可选 cache/reasoning/route 值缺失保持未知，不补零；reasoning/route 字段名未证实不映射",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "source_retention": "持久日志保留策略未文档化；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（type/usage 四键），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": DSH_PARSER_VERSION,
                "format_evidence": "固定 token-meter README 46a7f68（tokenUsage 四字段、final 替换、retry 边界、估算排除）；落盘路径与行序列化未文档化，JSONL 行形状为合成假设",
                "upgrade_policy": "未文档化事件 type / usage 形状偏离 fail closed；取得真实样本后扩展接受集与时间/模型字段",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "全部字段口径为文档级证据（固定 README 46a7f68，A08），本机无真实样本（not_found）；首份真实 fixture 到达后逐字段核验".into(),
                "持久日志落盘路径与行序列化未在 pinned README 记载：发现只接受手工根，JSONL 行形状为合成假设（fixtures 标 synthetic）".into(),
                "pinned README 未记载逐事件时间字段：occurred_at 用观察时间，日归属受此限制，待真实样本核验时间字段".into(),
                "attempt 可选 reasoning/route 值的字段名未证实：不映射不猜（缺省保持未知）".into(),
                "contextPressure/contextBreakdown 是估算与组合视图，不进入用量（一次性诊断可见）".into(),
                "日志尾部未闭合 attempt 以 Partial 入账；会话终止且无后续事件时保持 Partial（值正确，生命周期证据缺失）".into(),
                "非追加式改写（截断/重写）超出 append-only 合同：重扫确定性重放幂等，但改写删除的 attempt 靠墓碑排除，恢复再现时保持排除".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}
