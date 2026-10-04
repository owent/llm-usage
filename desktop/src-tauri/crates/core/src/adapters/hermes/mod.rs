//! Hermes Agent 适配器（独立目录约定 architecture.md#adapter-layout）：
//! - 本模块是该 Agent 的稳定入口（统一接口实现与再导出）；
//! - [`detect`]：state.db schema 指纹（真实列 + v22 主键形状）探测与版本分派；
//! - [`versions`]：格式实现注册与映射（`session_model_usage_v1` 区间汇总）；
//! - [`common`]：产品特有 usage 映射 + base_url 内存规范化 + 源库只读/暂存副本约定。
//!
//! 实现依据与核验范围（A24）：固定源码 commit ef70b3661cbfcf57e583008ad91dd04d8ba46070
//! （hermes_state_common.py SCHEMA_SQL / hermes_state_usage.py / agent/turn_usage.py）
//! 与官方存储文档；**本机未安装（2026-09-25 盘点 not_found），无真实样本**，
//! 能力声明与合成 fixtures 均标注"文档级证据、待真实样本"。
//! 首个可交付能力 = 本机按模型/任务的原生区间统计；逐次请求（agent 日志详单）
//! 与精确日统计独立标注，不在本实现范围。

pub mod common;
pub mod detect;
pub mod versions;

pub use common::{map_hermes, HermesUsage};
pub use detect::HERMES_FORMAT;
pub use versions::{session_model_usage_v1, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Hermes home 环境覆盖（固定源码 hermes_constants.py get_hermes_home）。
pub const HERMES_ENV_HOME: &str = "HERMES_HOME";

/// Hermes Agent 适配器（无状态）。
pub struct HermesAdapter;

impl Default for HermesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesAdapter {
    pub fn new() -> Self {
        HermesAdapter
    }

    /// 一个候选根下的 state.db 文件（根本身 + 命名 profile 一层）。
    /// profile 目录须匹配固定源码 PROFILE_ID_RE（^[a-z0-9][a-z0-9_-]{0,63}$）
    /// 且带身份标记（config.yaml/.env/SOUL.md/profile.yaml/auth.json/state.db）。
    fn state_dbs_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        use crate::adapters::framework::enumerate_files_bounded;
        // 深度 3 覆盖 root/profiles/<name>/state.db。
        let files = enumerate_files_bounded(root, 3, &|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "state.db")
        });
        let profile_id_ok = |dir: &std::path::Path| -> bool {
            let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            let name_ok = !name.is_empty()
                && name.len() <= 64
                && name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
            if !name_ok {
                return false;
            }
            [
                "config.yaml",
                ".env",
                "SOUL.md",
                "profile.yaml",
                "auth.json",
                "state.db",
            ]
            .iter()
            .any(|marker| dir.join(marker).exists())
        };
        files
            .into_iter()
            .filter(|p| {
                // 相对根深度：1 = 根直下 state.db（默认 home/HERMES_HOME 指向本身）；
                // 2 = 手工根宽松（用户 home/备份根下一层，身份由 detect 指纹把关）；
                // 3 = profiles/<name>/state.db（须 profile 语法；state.db 本身即
                // 固定源码身份标记之一，无须另验标记文件）。
                let Ok(rel) = p.strip_prefix(root) else {
                    return false;
                };
                match rel.components().count() {
                    1 | 2 => true,
                    3 => {
                        rel.components().next().and_then(|c| c.as_os_str().to_str())
                            == Some("profiles")
                            && p.parent().is_some_and(&profile_id_ok)
                    }
                    _ => false,
                }
            })
            .collect()
    }
}

impl crate::adapters::framework::SourceAdapter for HermesAdapter {
    fn adapter_id(&self) -> &'static str {
        "hermes"
    }

    fn agent(&self) -> &'static str {
        "hermes-agent"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // HERMES_HOME 覆盖（固定源码：上下文覆盖采集器不可见，取环境变量一级）。
        if let Some(home) = ctx.env.get(HERMES_ENV_HOME) {
            if !home.trim().is_empty() {
                roots.push((
                    std::path::PathBuf::from(home.trim()),
                    RootBasis::EnvOverride(HERMES_ENV_HOME.to_string()),
                ));
            }
        }
        // 平台默认（固定源码 _get_platform_default_hermes_home）：
        // Windows %LOCALAPPDATA%/hermes（缺省回退 ~/AppData/Local/hermes），
        // macOS/Linux ~/.hermes。
        if let Some(home) = &ctx.home_dir {
            if cfg!(windows) {
                let base = ctx
                    .env
                    .get("LOCALAPPDATA")
                    .filter(|v| !v.trim().is_empty())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home.join("AppData").join("Local"));
                roots.push((base.join("hermes"), RootBasis::DefaultHome));
            } else {
                roots.push((home.join(".hermes"), RootBasis::DefaultHome));
            }
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
            // 手工根语义宽松：接受 hermes home 本身、其父目录或用户 home
            //（有界深度 2 定位 state.db 与 profiles/<name>/state.db）。
            for file in Self::state_dbs_under(&root) {
                let Some(parent) = file.parent() else {
                    continue;
                };
                if !seen.insert(parent.to_path_buf()) {
                    continue;
                }
                out.push(DiscoveredRoot {
                    root: parent.to_path_buf(),
                    basis: basis.clone(),
                    files: vec![file],
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "hermes@{}",
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
        versions::session_model_usage_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting =
            "文档级证据（A24 固定源码）；本机 not_found（2026-09-25 盘点），待真实样本核验"
                .to_string();
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
                Availability::Partial(awaiting.clone()),
                "session_model_usage 五计数列（input/output/cache_read/cache_write/reasoning）逐列报告；包含关系未随 normalize_usage 验证，不推导互斥/总量",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "cache_read_tokens 列",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "cache_write_tokens 列",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Unavailable(
                    "session_model_usage 是模型/路由/任务累计，不是逐请求表；api_call_count 只作区间调用汇总，不拆 model_call"
                        .into(),
                ),
                "无（agent/turn_usage.py 日志详单待真实样本另验）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "组合键 model/billing_provider/billing_base_url/billing_mode/task（base_url 内存规范化）；区间汇总层当前无模型维度列",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Unavailable(
                    "first_seen/last_seen 是聚合写入边界（REAL epoch 秒），非逐请求时间；跨日累计按区间保存，无精确每日趋势"
                        .into(),
                ),
                "区间端点（uncertain）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable(
                    "estimated/actual_cost_usd 列存在但区间汇总载体无费用字段；本机调用归属明确前不映射（不读 Portal 账户）"
                        .into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("累计行无延迟信息（日志详单另验）".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "hermes".to_string(),
            product: "Hermes Agent（Nous Research）".to_string(),
            surfaces: vec!["cli".into(), "gateway".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": {
                    "windows": "%LOCALAPPDATA%/hermes（缺省回退 ~/AppData/Local/hermes）",
                    "unix": "~/.hermes",
                },
                "env_override": "HERMES_HOME",
                "manual_roots": true,
                "bounded": true,
                "pattern": "<hermes home>/state.db 与 <home>/profiles/<name>/state.db（profile 按固定源码身份标记）",
                "profile": "命名 profile 独立目录/state.db（固定源码 PROFILE_ID_RE）",
            }),
            detection: serde_json::json!({
                "magic": "SQLite + sessions/session_model_usage 真实列 + v22 六列主键（不只看 schema_version 整数）",
                "version_field": "schema_version 表单行整数（固定源码 SCHEMA_VERSION=30）",
                "registry": "adapters/hermes/versions 注册表分派（当前空：文档级证据）",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本一律 latest_fallback（带兼容标记）；pre-v22 主键/缺表缺列 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "interval_aggregate": "每行一条来源原生区间汇总（first_seen..last_seen，回填行回退 session 时间窗）；跨日不落单日、不摊分",
                "auxiliary": "task != '' 行为独立辅助累计（record_auxiliary_usage 只写任务键，不进主会话总量）：主 100 + 辅助 20 = 120",
                "absolute_path": "update_token_counts 绝对路径只覆盖 sessions 总量且不写模型行；本适配器只读 session_model_usage，不与 sessions 累计列叠加",
                "backfill": "v20 回填行（INSERT OR IGNORE from sessions）不证明历史调用使用该模型；NULL first_seen/last_seen 时区间端点取 session 时间窗",
                "compression_subagent": "压缩/子 Agent 是独立 session 行、各有模型行；无跨行继承双计（待真实样本验证覆盖集合）",
                "residuals": "未解释残差保持未知，不强归 session 当前模型",
            }),
            incremental: serde_json::json!({
                "cursor": "schema 指纹 + 六键 scope_key + 有效结束毫秒水位（+60s 有界重叠窗）",
                "row_cap": "单轮 50,000 行；触顶停在最后一个完整毫秒",
                "schema_evolution": "指纹变化 ⇒ 水位重置全量重读（scope_key upsert 幂等）",
                "no_change_detection": "WAL 活库不做字节长度短路；同键同内容聚合 upsert 无变更",
                "rescan_note": "in-place 页重写触发框架 Rescan 标记；水位不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "source_aggregates (instance, session scope, smu:<六键摘要>)；base_url 内存规范化不落原文",
                "cross_source": "不读 sessions 累计列/messages/FTS 正文；外部 Codex 镜像与 MoA 覆盖集合待真实样本验证",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "绝对路径写 sessions 不写模型行时，模型行覆盖缺口不可见（不补零）；缺时间/负计数行跳过并记诊断",
                "sampling": "累计表无采样概念；api_call_count 是来源调用汇总非事件数",
                "source_retention": "源端保留/prune 未知；可回填范围以现存行为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::session_model_usage_v1::HERMES_PARSER_VERSION,
                "format_evidence": "固定源码 ef70b3661cbfcf57e583008ad91dd04d8ba46070（SCHEMA_SQL/usage/turn_usage）+ 官方存储文档；合成 fixtures 标注待真实样本",
                "evidence_level": "doc-level（无本机真实样本；2026-09-25 盘点 not_found）",
                "upgrade_policy": "取得真实脱敏 fixture 后逐 schema_version 升为已验证；未收录版本 latest_fallback",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "水位窗口查询（有界行数）；每轮固定一次探测查询",
                "pause_cancel": "行级游标可停；busy 源转暂存副本或保留旧结果下轮重试",
            }),
            limitations: vec![
                "文档级证据实现：本机未安装（2026-09-25 盘点 not_found），全部版本 latest_fallback，待真实样本核验后升级".into(),
                "session_model_usage 是累计表：无逐次请求、无精确每日趋势；api_call_count 不拆成 model_call".into(),
                "input/cache、reasoning/output 包含关系未随 normalize_usage 完整路径验证：不推导互斥桶与总量，仅逐列报告".into(),
                "区间汇总层（source_aggregates）无模型/费用维度列：按模型统计与成本映射待数据层扩展；不读 Hermes Portal/account_usage".into(),
                "pre-v22 主键或缺表缺列 fail closed；v20 回填行语义（api_call_count 种子值）未逐字取证，按列面值处理".into(),
                "压缩继承/子 Agent/MoA/外部 Codex 镜像的覆盖集合未验证（待真实样本）；残差不强归当前模型".into(),
            ],
        }
    }
}
