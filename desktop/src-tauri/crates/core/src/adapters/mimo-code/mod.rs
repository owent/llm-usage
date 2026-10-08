//! MiMo Code adapter; see architecture.md#adapter-layout.
//! - Stable entry point implements the shared interface and re-exports modules.
//! - detect checks mimocode.db, including product-specific message.agent_id,
//!   and session.version; OpenCode ancestry does not establish compatibility.
//! - versions has no dedicated per-record mappings yet, so every version uses latest_fallback.
//! - common owns product-specific schema checks and read-only/staged source access.
//! - Wire parsing is shared in crate::adapters::opencode_family.
//!
//! References: official 0.1.15 distribution and commit 14dfe68a, checked with real
//! container/API/native SQLite samples; other paths/versions retain limits in m3-runtime-samples.md.
//! - resolveMimocodeHome uses absolute MIMOCODE_HOME + /data;
//!   otherwise $XDG_DATA_HOME/mimocode or default ~/.local/share/mimocode.
//!   WAL databases use mimocode.db or channel-specific mimocode-<channel>.db.
//! - Per-step usage comes from part step-finish rows; see family module references.
//!   Read step usage without adding message aggregates.

pub mod common;
pub mod detect;
pub mod versions;

pub use detect::MIMO_CODE_FORMAT;
pub use versions::step_finish_parts_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// resolveMimocodeHome requires an absolute MIMOCODE_HOME; upstream rejects relative values.
/// Discovery disables environment/default candidates for an invalid override; manual roots remain available.
pub const MIMOCODE_ENV_HOME: &str = "MIMOCODE_HOME";

/// Stateless MiMo Code adapter.
pub struct MimoCodeAdapter;

impl Default for MimoCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MimoCodeAdapter {
    pub fn new() -> Self {
        MimoCodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for MimoCodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "mimo-code"
    }

    fn agent(&self) -> &'static str {
        "mimo-code"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // Upstream uses one mode; a nonempty invalid HOME throws, it does not
        // silently fall through to an unrelated XDG/default source.
        let override_home = ctx.env.get(MIMOCODE_ENV_HOME).filter(|v| !v.is_empty());
        let data = if let Some(home) = override_home {
            std::path::Path::new(home).is_absolute().then(|| {
                (
                    std::path::PathBuf::from(home).join("data"),
                    RootBasis::EnvOverride(MIMOCODE_ENV_HOME.into()),
                )
            })
        } else if let Some(xdg) = ctx
            .env
            .get("XDG_DATA_HOME")
            .filter(|v| !v.is_empty() && std::path::Path::new(v).is_absolute())
        {
            Some((
                std::path::PathBuf::from(xdg).join("mimocode"),
                RootBasis::EnvOverride("XDG_DATA_HOME".into()),
            ))
        } else {
            ctx.home_dir
                .as_ref()
                .map(|home| (home.join(".local/share/mimocode"), RootBasis::DefaultHome))
        };
        // storage/db.ts: absolute DB overrides data; relative DB is under data.
        // :memory: has no persisted source. An invalid HOME remains invalid.
        if override_home.is_none() || data.is_some() {
            if let Some(db) = ctx.env.get("MIMOCODE_DB").filter(|v| !v.is_empty()) {
                if db != ":memory:" {
                    let path = std::path::PathBuf::from(db);
                    if path.is_absolute() {
                        roots.push((path, RootBasis::EnvOverride("MIMOCODE_DB".into())));
                    } else if let Some((data, _)) = &data {
                        roots.push((
                            data.join(path),
                            RootBasis::EnvOverride("MIMOCODE_DB".into()),
                        ));
                    }
                }
            } else if let Some(data) = data {
                roots.push(data);
            }
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        // Accept explicit database files; directory candidates enumerate mimocode*.db without disk-wide recursion.
        // Manual directory roots may directly contain databases or provide <root>/data
        // (MIMOCODE_HOME layout), <root>/mimocode or <root>/.local/share/mimocode.
        // Filename prefixes do not distinguish related products;
        // detect must verify the MiMo-specific schema before reading usage.
        let is_db_name = |p: &std::path::Path| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("mimocode") && n.ends_with(".db"))
                .unwrap_or(false)
        };
        let has_db = |dir: &std::path::Path| {
            !crate::adapters::framework::enumerate_files_bounded(dir, 0, &is_db_name).is_empty()
        };
        let mut out: Vec<DiscoveredRoot> = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (root, basis) in roots {
            if root.is_file() {
                let Some(parent) = root.parent() else {
                    continue;
                };
                if seen.insert(crate::adapters::framework::normalize_path(&root)) {
                    out.push(DiscoveredRoot {
                        root: parent.to_path_buf(),
                        basis,
                        files: vec![root.clone()],
                    });
                }
                continue;
            }
            if !root.is_dir() {
                continue;
            }
            let mut data_dirs: Vec<std::path::PathBuf> = Vec::new();
            if has_db(&root) {
                data_dirs.push(root.clone());
            }
            for rel in [
                std::path::PathBuf::from("data"),
                std::path::PathBuf::from("mimocode"),
                std::path::PathBuf::from(".local")
                    .join("share")
                    .join("mimocode"),
            ] {
                let candidate = root.join(rel);
                if candidate.is_dir() && has_db(&candidate) {
                    data_dirs.push(candidate);
                }
            }
            for data_dir in data_dirs {
                let files =
                    crate::adapters::framework::enumerate_files_bounded(&data_dir, 0, &is_db_name)
                        .into_iter()
                        .filter(|p| seen.insert(crate::adapters::framework::normalize_path(p)))
                        .collect::<Vec<_>>();
                if files.is_empty() {
                    continue;
                }
                out.push(DiscoveredRoot {
                    root: data_dir,
                    basis: basis.clone(),
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "mimo-code@{}",
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
        // All attempted versions use step_finish_parts_v1; extend dispatch with the registry.
        versions::step_finish_parts_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let awaiting =
            "real-container 0.1.15（固定源码 14dfe68a），仅 CLI/续会话 OpenAI-compatible 本地模型"
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
                "0.1.15 自身 SDK 归一：input=SDK 输入减缓存、output=SDK 输出减 reasoning；加回同份分项恢复正 SDK 总量，默认零子桶未知；只读 part，与正 SDK total 对照",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "tokens.cache.read 列",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "tokens.cache.write 列",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "每条 step-finish 部件 = 一次 step 的模型调用（part.id 稳定身份，time_updated 修订序号）；step 与 message 汇总不双计（只读 step 侧）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "join message.data.$.modelID/providerID（request_field；message-v2.ts Assistant zod 必需字段）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "part.time_created 是投影写入时刻（≈step 完成，observed_at），非上游精确逐调用时间",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(awaiting.clone()),
                "step-finish 正 cost 为上游自算 estimated micro-USD；默认零未知，真实本地模型无正费用，不认证账单",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("step-finish 部件无延迟/TTFT 字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "mimo-code".to_string(),
            product: "MiMo Code（XiaomiMiMo）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["MIMOCODE_HOME/data", "$XDG_DATA_HOME/mimocode", "~/.local/share/mimocode"],
                "env_override": ["MIMOCODE_HOME（非空最高优先，须绝对路径；无效不回退默认目录）", "XDG_DATA_HOME", "MIMOCODE_DB（绝对或相对 data 的任意文件名；:memory: 无落盘）"],
                "manual_roots": "直接库文件 / 数据目录 / <root>/data / <root>/mimocode / <root>/.local/share/mimocode；物理文件别名去重",
                "bounded": true,
                "pattern": "<data>/mimocode*.db（通道变体 mimocode-<channel>.db；-wal/-shm 排除）",
                "profile": "无 profile 概念（固定源码未见）",
            }),
            detection: serde_json::json!({
                "magic": "SQLite + part/session/message 关键列（schema 指纹；message 须含 agent_id 列 ⇒ 与 OpenCode 库互斥，不从 fork 关系推兼容）",
                "version_field": "事件所属 session.version；库内最大值不认证其他会话",
                "registry": "与 opencode 独立；当前全部 latest_fallback，不从单一路线样本认证整版本",
                "fail_closed": true,
                "unknown_version": "未收录/缺失版本一律 latest_fallback（带兼容标记）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "step-finish 部件逐次入账（part.id 身份，time_updated 修订序号）",
                "no_double_count": "只读 part 表；assistant message.data.tokens 是 turn 级聚合不读（A14 step 与 message 汇总不双计）",
                "no_session_counters": "MiMo session 表无 tokens_* 累计列（session.sql.ts）：无会话级对账目标，残差不可见",
                "subagent": "session.parent_id 非空 ⇒ sub_agent；message.agent_id（默认 main）为产品特有列，语义未证实不用于分类",
            }),
            incremental: serde_json::json!({
                "cursor": "schema 指纹 + part.id 稳定键 + time_updated 处理位置（+60s 有界重叠窗）",
                "row_cap": "单轮 50,000 行；触顶停在最后一个完整毫秒",
                "schema_evolution": "指纹变化 ⇒ 处理位置重置全量重读（id 键 upsert 幂等）",
                "no_change_detection": "WAL 活库：游标 offset 恒 0，每轮执行处理位置查询而非字节短路",
                "rescan_note": "in-place 页重写触发框架 Rescan 标记；处理位置不重置（同一逻辑库）",
            }),
            dedup: serde_json::json!({
                "primary": "mimo-code:part:{part.id}（实例命名空间，与 opencode 分列）",
                "cross_source": "不读 event/同步层与外部导入表；与 OpenCode/kilo 数据根不重叠、注册表独立",
                "rescan": "重扫/重叠窗重复行按同键同修订幂等（unchanged），不双计",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "被删部件/未完成 step 缺证据不计、不补零；MiMo 无 session 累计列，对账缺口不可见（如实标注）",
                "sampling": "未观测到采样；坏 data 行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存行时间为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::step_finish_parts_v1::PARSER_VERSION,
                "format_evidence": "官方 0.1.15/14dfe68a 自身 session getUsage、SQLite DDL、路径/DB 覆盖与八次真实 API/part 白名单对照",
                "evidence_level": "real-container 0.1.15 CLI/resume，本地 OpenAI-compatible；其他版本/协议未验证",
                "upgrade_policy": "旧处理位置重评；完整旧摘要限定默认零/解析依据修正，修订/身份/真实冲突与诊断历史保留",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "time_updated 处理位置查询；每轮固定一次探测查询",
                "pause_cancel": "行级游标可停；busy 源转暂存副本或保留旧结果下轮重试",
            }),
            limitations: vec![
                "仅 0.1.15 CLI/续会话 OpenAI-compatible 路线真实核对；其他版本、协议及非 main 角色待验，全部 latest_fallback".into(),
                "八次 finish reason 为 length，CLI 退出 0 不认证任务完成；默认缓存/推理/费用零未知".into(),
                "session 表无累计列：无会话级对账，逐次合计与上游总量的残差不可见（不虚构对账目标）".into(),
                "message.agent_id 语义（'main' 之外取值）未经真实样本证实，不用于调用分类".into(),
                "MIMOCODE_HOME 非绝对路径时上游抛错：默认发现不回退 XDG/HOME，显式手工根仍可接入".into(),
                "step 与 message 汇总不双计：只读 part；turn 级聚合不重复入账".into(),
            ],
        }
    }
}
