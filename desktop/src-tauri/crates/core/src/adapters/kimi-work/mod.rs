//! Kimi Work adapter; see architecture.md#adapter-layout, M4/A13.
//! - Stable entry point implements the shared interface and re-exports modules.
//! - detect reads the initial metadata header and protocol_version,
//!   with registered format 1.4.
//! - versions registers verified formats; unregistered values, including 1.5,
//!   or missing versions use the latest built-in parser as fallback.
//! - Wire parsing is shared in crate::adapters::kimi_wire; roots and instance
//!   identities, version registries and Agent statistics stay product-specific (adapters.md).
//!
//! Product references: embedded kimi-code home under the daimon host directory,
//! state.json createdBy=daimon-kernel-adapter, separate from standalone Kimi Code
//! default ~/.kimi-code data.

pub mod detect;
pub mod versions;

pub use detect::KIMI_WORK_FORMAT;
pub use versions::wire_v14::PARSER_VERSION as KIMI_WORK_PARSER_VERSION;
pub use versions::{wire_v14, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Observed embedded-home candidate: 69 files/31.5 MiB in the 2026-09-25 local inventory.
/// Derived from this installation with custom D:/Cache; not an official default.
/// No documented environment override was found; other locations use manual_roots.
pub const KIMI_WORK_OBSERVED_HOME: &str =
    "D:/Cache/Kimi/share/daimon-share/daimon/runtime/kimi-code/home";

/// Stateless Kimi Work adapter.
pub struct KimiWorkAdapter;

impl Default for KimiWorkAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KimiWorkAdapter {
    pub fn new() -> Self {
        KimiWorkAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for KimiWorkAdapter {
    fn adapter_id(&self) -> &'static str {
        "kimi-work"
    }

    fn agent(&self) -> &'static str {
        "kimi-work"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // Candidate pairs contain sessions directory and basis; the instance root is that directory.
        // Candidates are the observed local layout and manual_roots.
        // Default candidates require a matching home context; tests or isolated callers
        // without it use manual roots, following the framework convention for claude/pi/qwen.
        // No documented environment override: do not read KIMI_CODE_HOME for this product.
        let mut candidates: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // This custom absolute installation belongs to the current process user.
        // A caller supplying an isolated home must never discover personal sources.
        let process_home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        if cfg!(windows)
            && ctx
                .home_dir
                .as_ref()
                .zip(process_home.as_ref())
                .is_some_and(|(home, real)| {
                    crate::adapters::framework::normalize_path(home).eq_ignore_ascii_case(
                        &crate::adapters::framework::normalize_path(std::path::Path::new(real)),
                    )
                })
        {
            candidates.push((
                std::path::PathBuf::from(KIMI_WORK_OBSERVED_HOME).join("sessions"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            // A manual root containing sessions is an embedded home; otherwise use the root as sessions.
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
            // Bounded depth 4 covers sessions/<wd>/<conv-*|ctitle-*>/agents/*/wire.jsonl.
            // Filename filtering excludes state.json, output.log and content-hash files.
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
            "kimi-work@{}",
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
        // All attempted versions use wire_v14; extend dispatch when the registry has more implementations.
        versions::wire_v14::scan(target, stored, limits, now_ms)
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
                "usage.record 四字段互斥无 total（与 kimi-code 1.5 同族实读）：inputOther/output/inputCacheRead/inputCacheCreation；input_total/total_tokens 派生",
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
                "每条 usage.record 一次调用（turn=主循环、session=压缩摘要辅助）；step.end 回声不计账；1.4 本机未观测 subagent.completed（子代理仅目录形态）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "usage.record.model（1.4 为裸 id 如 k3-agent-swarm/daimon-kimi-code，原样 model_raw）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "usage.record.time epoch 毫秒（本机 69 文件实读全部毫秒）；source_completion 口径",
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
                    "延迟/TTFT 只在 step.end 回声侧，与 usage.record 无稳定关联键，不并账".into(),
                ),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "kimi-work".to_string(),
            product: "Kimi Work（daimon 宿主内嵌 kimi-code 内核）".to_string(),
            surfaces: vec!["desktop".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [format!("{KIMI_WORK_OBSERVED_HOME}/sessions")],
                "default_root_basis": "本机实测安装位推导（D:/Cache 为自定义缓存盘），非官方文档默认；官方环境覆盖未见文档 ⇒ 不读 env",
                "env_override": null,
                "manual_roots": "含 sessions 子目录按内嵌 home 解析，否则按 sessions 目录本身",
                "bounded": true,
                "pattern": "sessions/<wd>/<conv-<hexid>|ctitle-<uuid>>/agents/<agent>/wire.jsonl（与 kimi-code 的 session_<uuid> 布局不同；同目录 state.json/output.log/hash 文件不读）",
                "profile": "无 profile 概念（本机未观测）",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=metadata",
                "version_field": "metadata.protocol_version（字符串；本机 69/69=\"1.4\"）",
                "registry": "adapters/kimi-work/versions 注册表分派（锚点 1.4，与 kimi-code 注册表独立：1.5 在本产品为 latest_fallback）",
                "fail_closed": true,
                "unknown_version": "未收录/缺失 protocol_version 先尝试最新内置解析器（latest_fallback）；无有证据不兼容版本",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "usage.record(usageScope=turn)（final；无 uuid/messageId，身份=time+同毫秒序号）",
                "auxiliary": "usage.record(usageScope=session) = 压缩摘要调用（本机 8 条实读，agent-44 fixture 含 1 条）",
                "echo_dedup": "step.end.event.usage 是 usage.record 逐字段回声（本机 1329/1329）：二选一，只计记录侧",
                "subagent": "agents/agent-N/ 独立 wire 逐次入账（sub_agent，1.4 无 agentId 字段 ⇒ 身份来自目录）；本机 1.4 未观测 subagent.completed/spawned 记录（子代理关系只在宿主层）",
                "micro_compaction": "micro_compaction.apply 是控制记录，不计账（本机 2 条实读）",
                "token_counting": "token_counting.* 不计账（同家族口径）",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
                "source_retention": "源端保留未知（最新 2026-09-13，宿主可能清理）；可回填以现存文件为准",
            }),
            dedup: serde_json::json!({
                "primary": "kimi-work:usage:{session 目录}:{agent}:{time 毫秒}:{同毫秒序号}（实例命名空间；本机实测 2 对跨文件同毫秒记录——swarm 并行子代理与主线同毫秒完成——键含 session/agent 段不撞）",
                "cross_source": "与 kimi-code 数据根/实例身份天然分离（kimi-work@<内嵌home> vs kimi-code@~/.kimi-code）；session_index.jsonl/state.json/output.log 不读",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "无 usage.record 的调用不可见；1.4 子代理生命周期不在 wire 内（宿主层），不影响逐次计账",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "no_timestamp": "time 超出合理毫秒域记诊断跳过，不做秒/毫秒猜测换算（实读全为毫秒）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::wire_v14::PARSER_VERSION,
                "format_evidence": "本机 Kimi Work 内嵌 home 69 文件实读（M4 真实脱敏 fixture conv-main/agent-44-subagent）；daimon 宿主、createdBy=daimon-kernel-adapter 佐证产品身份；无固定上游源码锚点",
                "upgrade_policy": "未收录 protocol_version（含 1.5）latest_fallback 兼容尝试；逐版本真实 fixture 核验后升为已验证；解析器升级不自动重扫已消费游标",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界；不启动 Agent",
            }),
            limitations: vec![
                "发现候选只有本机实测布局 + manual_roots：安装位迁移后需手工添加（官方默认布局与环境覆盖未见文档）".into(),
                "usage.record 无 agentId（1.4 实读）：代理身份完全依赖 agents/<id>/ 目录名".into(),
                "usage.record 无 uuid/messageId：事件键=session目录+agent+time+同毫秒序号（本机实测 2 对跨文件同毫秒记录，键含身份段不撞）".into(),
                "provider 与逐次延迟在 llm.request/step.end 回声侧，与记录无关联键，不入账".into(),
                "除 1.4 外版本未取证（1.5 在本产品走 latest_fallback）".into(),
                "1.4 未观测 subagent.completed；实现仍防双计（出现时不产事件），待真实样本核验".into(),
                "符号链接/junction 不跟随；Windows 身份靠创建时间+首采样".into(),
            ],
        }
    }
}
