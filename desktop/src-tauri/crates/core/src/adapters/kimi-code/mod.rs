//! 新版 Kimi Code 适配器（独立目录约定 architecture.md#adapter-layout，M4/A12）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：产品/格式探测与版本分派（首行 metadata 头 + protocol_version）；
//! - [`versions`]：已验证格式实现的注册与映射；未收录/缺失版本默认回退最新
//!   内置解析器（尚无已确认不兼容的版本）；
//! - wire 解析逻辑在家族共享模块 [`crate::adapters::kimi_wire`]（与 Kimi Work
//!   经真实数据测试证明一致的部分）；数据根、实例身份、注册表锚点与统计分列
//!   独立（adapters.md：不因内核同名合并）。
//!
//! 格式依据：本机 desktop 1.0.3、wire protocol_version=1.5（M0 fixture +
//! tests/fixtures/kimi-code 真实脱敏样本，2026-09-25）。

pub mod detect;
pub mod versions;

pub use detect::KIMI_CODE_FORMAT;
pub use versions::wire_v15::PARSER_VERSION as KIMI_CODE_PARSER_VERSION;
pub use versions::{wire_v15, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// 官方确认的环境覆盖（adapters.md A12：KIMI_CODE_HOME/sessions/...）。
pub const KIMI_CODE_ENV_HOME: &str = "KIMI_CODE_HOME";

/// Kimi Code 适配器（无状态）。
pub struct KimiCodeAdapter;

impl Default for KimiCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KimiCodeAdapter {
    pub fn new() -> Self {
        KimiCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for KimiCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "kimi-code"
    }

    fn agent(&self) -> &'static str {
        "kimi-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // (sessions 目录, basis)；root 统一取 sessions 目录，跨来源去重后同目录只扫一次。
        let mut candidates: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get(KIMI_CODE_ENV_HOME) {
            candidates.push((
                std::path::PathBuf::from(home).join("sessions"),
                RootBasis::EnvOverride(KIMI_CODE_ENV_HOME.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            candidates.push((
                home.join(".kimi-code").join("sessions"),
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
            // sessions/<wd>/session_*/agents/*/wire.jsonl：深度 4（agent 目录一层），有界枚举；
            // 同目录其他文件（hash@v1、bash-*.json、state.json、日志）按名过滤不读。
            let files = crate::adapters::framework::enumerate_files_bounded(&sessions, 4, &|p| {
                p.file_name().and_then(|n| n.to_str()) == Some("wire.jsonl")
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
            "kimi-code@{}",
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
        // 当前所有可尝试版本共用 wire_v15；注册表扩展多实现后在此按选择分派。
        versions::wire_v15::scan(target, stored, limits, now_ms)
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
                "usage.record 四字段互斥无 total（M0 实读）：inputOther=未缓存输入、inputCacheRead=缓存读、inputCacheCreation=缓存写、output；input_total/total_tokens 为派生和",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "inputCacheRead reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Available,
                "inputCacheCreation reported（本机样本全 0，字段映射经合成样本验证）",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "每条 usage.record 一次调用（turn=主循环、session=压缩摘要辅助）；step.end 回声与 subagent.completed 快照不计账",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "usage.record.model（1.5 为 alias/model 组合串，原样 model_raw）；provider 仅在 llm.request 且无关联键，不入账",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "usage.record.time epoch 毫秒（本机 13 文件实读全部毫秒）；source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("wire 无费用字段；远端账单/额度页不接入".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable(
                    "延迟/TTFT 只在 step.end 回声（llmStreamDurationMs/llmFirstTokenLatencyMs），与 usage.record 无稳定关联键，不并账".into(),
                ),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "kimi-code".to_string(),
            product: "Kimi Code（desktop）".to_string(),
            surfaces: vec!["desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.kimi-code/sessions"],
                "env_override": [KIMI_CODE_ENV_HOME],
                "manual_roots": "含 sessions 子目录按 agent 根解析，否则按 sessions 目录本身",
                "bounded": true,
                "pattern": "sessions/<wd>/session_<uuid>/agents/<agent>/wire.jsonl（同目录 hash@vN、bash-*.json、state.json、日志不读）",
                "profile": "无 profile 概念（本机未观测）",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=metadata",
                "version_field": "metadata.protocol_version（字符串；本机 13 文件 = 11×\"1.5\" + 2×\"1.4\"（旧会话走 latest_fallback 兼容尝试））",
                "registry": "adapters/kimi-code/versions 注册表分派（与 kimi-work 注册表独立）",
                "fail_closed": true,
                "unknown_version": "未收录/缺失 protocol_version 先尝试最新内置解析器（latest_fallback）；无有证据不兼容版本",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "usage.record(usageScope=turn)（final；无 uuid/messageId，身份=time+同毫秒序号）",
                "auxiliary": "usage.record(usageScope=session) = full_compaction 压缩摘要调用（本机 3 条实读：区间内 llm.request kind=compaction 对应）",
                "echo_dedup": "context.append_loop_event 的 step.end.event.usage 是 usage.record 逐字段回声（M0）：二选一，只计记录侧；打断步可能无回声（本机 958/960）",
                "subagent": "agents/agent-N/ 独立 wire 逐次入账（sub_agent）；主线 subagent.completed.usage 是截至其 time 的 Σ 快照，不产事件（M0+本机逐字段复证相等）",
                "subagent_reuse": "completed 之后子代理 wire 可继续增长（本机 agent-0 观测：后 128 条属复用段，仍逐次入账）",
                "error_steps": "error/打断步无 usage.record（usage:null）⇒ 不计 token 不计调用（无调用完成证据）",
                "token_counting": "token_counting.* 是上下文估算/累计表，不是逐次 usage，不计账",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
                "live_file": "wire 为活文件（M0 采样期间 394→515 行）；增量续读天然适配",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            dedup: serde_json::json!({
                "primary": "kimi-code:usage:{session 目录}:{agent}:{time 毫秒}:{同毫秒序号}（实例命名空间；session/agent 段防跨文件同毫秒撞键）",
                "echo_snapshots": "step.end 回声与 subagent.completed 快照在源头即不产事件，无双计面",
                "cross_source": "与 kimi-work 数据根不重叠（~/.kimi-code vs 内嵌 home）；session_index.jsonl/logs/telemetry 不读",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "无 usage.record 的调用（如被打断步）不可见；error 步既有 evidence 不伪造",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "no_timestamp": "time 超出合理毫秒域记诊断跳过，不做秒/毫秒猜测换算（实读全为毫秒）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::wire_v15::PARSER_VERSION,
                "format_evidence": "本机 desktop 1.0.3 实读（M0 fixture + M4 真实脱敏 fixture session-main/subagent-agent-0）；无固定上游源码锚点，协议仅以本机观测为证",
                "upgrade_policy": "未收录 protocol_version latest_fallback 兼容尝试；逐版本真实 fixture 核验后升为已验证；解析器升级不自动重扫已消费游标",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界；不启动 Agent",
            }),
            limitations: vec![
                "usage.record 无 uuid/messageId：事件键=session目录+agent+time+同毫秒序号（确定性且重扫稳定；上游未来加 ID 后可平滑换键，旧键事件不迁移）".into(),
                "provider 与逐次延迟字段在 llm.request/step.end 回声侧，与 usage.record 无稳定关联键，均不入账".into(),
                "usageScope 语义（turn/session）来自本机 965 条实读归纳；新 scope 值记诊断跳过不猜".into(),
                "除 1.5 外版本未取证（1.4 在本产品走 latest_fallback）".into(),
                "子代理目录名!=main 即判 sub_agent（agentId 字段缺席时按目录；1.5 主线带 agentId 一致）".into(),
                "符号链接/junction 不跟随；Windows 身份靠创建时间+首采样".into(),
                "latest_fallback 文件的解析器升级后不自动重扫已消费游标；显式重扫可重新尝试".into(),
            ],
        }
    }
}
