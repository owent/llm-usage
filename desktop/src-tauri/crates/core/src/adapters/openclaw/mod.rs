//! OpenClaw 适配器（独立目录合同 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：文档形状分类 + fail closed（见 detect 模块头）；
//! - [`versions`]：注册表（空集，待真实样本）与运行时库解析占位；
//! - [`common`]：源库只读/暂存副本合同（复制自 kilo，目录独立）。
//!
//! 证据级别（A09 官方文档，2026-09-24 核验）：每 Agent 一个
//! `~/.openclaw/agents/<agentId>/agent/openclaw-agent.sqlite`（会话行 +
//! 追加式 transcript）；旧 `sessions/` 目录为迁移/归档输入。
//! **本机未安装（2026-09-25 盘点 not_found），文档未给出表级 schema**：
//! 本适配器交付发现/身份/诚实 fail closed 与能力声明，不猜字段、不产零值；
//! 真实样本取证后在 versions/runtime_store 实现读取映射。

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::OPENCLAW_FORMAT;
pub use versions::{runtime_store, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// OpenClaw 适配器（无状态）。
pub struct OpenClawAdapter;

impl Default for OpenClawAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenClawAdapter {
    pub fn new() -> Self {
        OpenClawAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for OpenClawAdapter {
    fn adapter_id(&self) -> &'static str {
        "openclaw"
    }

    fn agent(&self) -> &'static str {
        "openclaw"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{enumerate_files_bounded, DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // 官方文档只给出 ~/.openclaw 默认根；无已证实的环境覆盖（不猜 env 名）。
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".openclaw"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out: Vec<DiscoveredRoot> = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            if !root.is_dir() {
                continue;
            }
            // 手工根语义宽松：接受 ~/.openclaw 根、agents/、单个 agent 目录或
            // 其父目录（有界深度 3 按文档形状定位，不递归整盘）。
            let files = enumerate_files_bounded(&root, 3, &|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                name == "openclaw-agent.sqlite"
                    || name == "sessions.json"
                    || name.ends_with(".jsonl")
            });
            // 按文档路径形状过滤并归组到 agents/<agentId> 实例根。
            let mut by_agent: std::collections::BTreeMap<
                std::path::PathBuf,
                Vec<std::path::PathBuf>,
            > = std::collections::BTreeMap::new();
            for file in files {
                fn os_to_str(c: &std::ffi::OsStr) -> Option<&str> {
                    c.to_str()
                }
                let components: Vec<&std::ffi::OsStr> =
                    file.components().map(|c| c.as_os_str()).collect();
                // 找 "agents" 段：其后第一段是 agentId，第二段须为 agent/ 或 sessions/。
                let mut agent_dir: Option<std::path::PathBuf> = None;
                'outer: for start in 0..components.len() {
                    if os_to_str(components[start]) != Some("agents") {
                        continue;
                    }
                    if let (Some(_id), Some(layer)) =
                        (components.get(start + 1), components.get(start + 2))
                    {
                        match os_to_str(layer) {
                            Some("agent") | Some("sessions") => {
                                agent_dir = Some(
                                    file.components()
                                        .take(start + 2)
                                        .collect::<std::path::PathBuf>(),
                                );
                                break 'outer;
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(agent) = agent_dir {
                    by_agent.entry(agent).or_default().push(file);
                }
            }
            for (agent_dir, mut files) in by_agent {
                if !seen.insert(agent_dir.clone()) {
                    continue;
                }
                files.sort();
                out.push(DiscoveredRoot {
                    root: agent_dir,
                    basis: basis.clone(),
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "openclaw@{}",
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
        versions::runtime_store::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting = Availability::Unavailable(
            "官方文档未给出表级 schema（A09 具体表待验）；本机 not_found（2026-09-25 盘点）；\
             fail closed 待真实样本"
                .into(),
        );
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
                awaiting.clone(),
                "文档证实会话行有 token counters、transcript 条目持久化规范化 usage；表/列名未文档化",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(awaiting.clone(), "同上（未文档化）"),
        );
        fields.insert(
            "cache_write".into(),
            field(awaiting.clone(), "同上（未文档化）"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                awaiting.clone(),
                "文档证实追加式 transcript 含 usage 测量；条目级 schema 未文档化",
            ),
        );
        fields.insert("model".into(), field(awaiting.clone(), "未文档化"));
        fields.insert("time".into(), field(awaiting.clone(), "未文档化"));
        fields.insert(
            "cost".into(),
            field(
                awaiting.clone(),
                "文档证实 usage.cost（记录金额或本地估算）；载体 schema 未文档化",
            ),
        );
        fields.insert("latency".into(), field(awaiting.clone(), "未文档化"));
        CapabilityTable {
            adapter_id: "openclaw".to_string(),
            product: "OpenClaw".to_string(),
            surfaces: vec!["gateway".into(), "cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["<home>/.openclaw"],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "agents/<agentId>/agent/openclaw-agent.sqlite（运行时）与 agents/<agentId>/sessions/（旧归档/迁移输入）",
                "per_agent": "每 Agent 一个库（官方 store 参考）",
            }),
            detection: serde_json::json!({
                "magic": "文档路径形状（agents/<agentId>/agent|sessions/...）+ SQLite 结构",
                "version_field": null,
                "registry": "adapters/openclaw/versions（空集，待真实样本）",
                "fail_closed": true,
                "unknown_version": "全部输入 fail closed：运行时库表级 schema 未文档化；旧归档按迁移输入降级（gateway 启动不导入，openclaw doctor --fix 迁移）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "runtime_store": "会话行为可变运行态（token counters）、transcript 为追加式树（id+parentId）；具体生命周期待表级取证",
                "legacy_archive": "旧 JSONL/sessions.json 为迁移/离线维护输入，不按运行态解析",
            }),
            incremental: serde_json::json!({
                "cursor": "待实现（当前无解析器）",
                "row_cap": "待实现",
                "note": "fail closed 输入不保存游标；重复扫描只重复诊断，不产生数据",
            }),
            dedup: serde_json::json!({
                "primary": "待实现（无解析器）",
                "cross_source": "外部 CLI 镜像（bound imports 保留本地 import owner）与多 agent 库分开实例；远端 Gateway 主机上的存储不属于本机（不采集）",
                "rescan": "无数据产出，无重扫双计风险",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "fail closed 不读内容；不产零值记录",
                "sampling": "未知",
                "source_retention": "未知（store 维护/保留策略见官方 store maintenance 文档，未核验数值）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::runtime_store::OPENCLAW_PARSER_VERSION,
                "format_evidence": "官方文档 store/token-use（A09，2026-09-24 核验）；无表级 schema、无本机样本",
                "evidence_level": "doc-level（本机 not_found 2026-09-25 盘点；待真实样本）",
                "upgrade_policy": "取得真实脱敏 fixture（openclaw-agent.sqlite + 旧归档样本）后在 versions/runtime_store 实现并登记版本",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "发现层有界枚举 + 结构探测；无行级读取",
                "pause_cancel": "fail closed 输入即时返回，无长事务",
            }),
            limitations: vec![
                "文档级证据实现：官方文档未给出任何表名/列名（A09 具体表待验），运行时库与旧归档均 fail closed，不猜字段、不产零值".into(),
                "本机未安装（2026-09-25 盘点 not_found）：无真实样本，能力声明按文档级标注".into(),
                "旧 sessions/ 目录是迁移/归档输入：Gateway 启动不导入（须 openclaw doctor --fix），本适配器按降级输入处理并标注待证".into(),
                "远端 Gateway 主机上的 ~/.openclaw 不属于本机来源，不在采集范围；外部 CLI 镜像去重待真实样本验证".into(),
                "usage 归一形状（input/output 别名、total 回退、usage.cost）已有文档证据，待表级 schema 取证后接入".into(),
            ],
        }
    }
}
