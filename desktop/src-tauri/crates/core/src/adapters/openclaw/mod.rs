//! Native schema 24 reader; official distribution and real CLI verification are
//! independent of mutable whole-database client version metadata.
pub mod common;
pub mod detect;
pub mod versions;
pub use detect::OPENCLAW_FORMAT;
pub use versions::{runtime_store, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

#[derive(Default)]
pub struct OpenClawAdapter;
impl OpenClawAdapter {
    pub fn new() -> Self {
        Self
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
        use crate::adapters::framework::{
            enumerate_files_bounded, normalize_path, DiscoveredRoot, RootBasis,
        };
        use std::path::PathBuf;
        let home = ctx
            .env
            .get("OPENCLAW_HOME")
            .filter(|v| !v.trim().is_empty())
            .map(|v| PathBuf::from(v.trim()))
            .or_else(|| ctx.home_dir.clone());
        let expand = |value: &str| {
            let value = value.trim();
            if value == "~" {
                return home.clone().unwrap_or_else(|| PathBuf::from(value));
            }
            if value.starts_with("~/") || value.starts_with("~\\") {
                if let Some(home) = &home {
                    return home.join(&value[2..]);
                }
            }
            PathBuf::from(value)
        };
        let mut roots = Vec::new();
        if let Some(value) = ctx
            .env
            .get("OPENCLAW_STATE_DIR")
            .filter(|v| !v.trim().is_empty())
        {
            roots.push((
                expand(value),
                RootBasis::EnvOverride("OPENCLAW_STATE_DIR".into()),
            ));
        } else if let Some(home) = home {
            let root = if home.join(".openclaw").exists() || !home.join(".clawdbot").exists() {
                home.join(".openclaw")
            } else {
                home.join(".clawdbot")
            };
            let basis = if ctx
                .env
                .get("OPENCLAW_HOME")
                .is_some_and(|v| !v.trim().is_empty())
            {
                RootBasis::EnvOverride("OPENCLAW_HOME".into())
            } else {
                RootBasis::DefaultHome
            };
            roots.push((root, basis));
        }
        roots.extend(
            ctx.manual_roots
                .iter()
                .cloned()
                .map(|root| (root, RootBasis::Manual)),
        );
        let mut grouped = std::collections::BTreeMap::<String, DiscoveredRoot>::new();
        for (root, basis) in roots {
            let accept =
                |path: &std::path::Path| detect::classify(path) != detect::InputKind::Other;
            let files = if root.is_file() && accept(&root) {
                vec![root]
            } else {
                enumerate_files_bounded(&root, 4, &accept)
            };
            for file in files {
                if let Some(agent) = detect::agent_dir(&file) {
                    let entry =
                        grouped
                            .entry(normalize_path(&agent))
                            .or_insert_with(|| DiscoveredRoot {
                                root: agent,
                                basis: basis.clone(),
                                files: Vec::new(),
                            });
                    if !entry
                        .files
                        .iter()
                        .any(|p| normalize_path(p) == normalize_path(&file))
                    {
                        entry.files.push(file);
                    }
                }
            }
        }
        grouped
            .into_values()
            .map(|mut root| {
                root.files.sort();
                root
            })
            .collect()
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
        runtime_store::scan(target, stored, limits, now_ms)
    }
    fn should_scan_unchanged(&self, _: &crate::adapters::framework::StoredScanState) -> bool {
        true
    }
    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let partial = Availability::Partial(
            "已核验 openai-completions 正桶；默认零未知，其他 transport 待验".into(),
        );
        let unavailable = Availability::Unavailable("待逐条来源依据核验".into());
        let mut fields = serde_json::Map::new();
        for (key, availability, note) in [
            (
                "tokens",
                partial.clone(),
                "input=非缓存输入；output=总输出；计算 totalTokens 不认证完整总量",
            ),
            (
                "cache_read",
                partial.clone(),
                "有效正桶；缺字段与默认零未知",
            ),
            ("cache_write", partial, "有效正桶；缺字段与默认零未知"),
            (
                "per_request_calls",
                unavailable.clone(),
                "assistant usage observation，不将条目数推导为底层调用数",
            ),
            (
                "model",
                Availability::Available,
                "仅记录自身 provider/model；不继承会话最新模型",
            ),
            (
                "time",
                Availability::Partial("当前 OpenAI transport timestamp 为请求开始".into()),
                "message.timestamp；不使用落库时刻补造完成时间",
            ),
            ("cost", unavailable.clone(), "本地默认估价零不认证账单"),
            ("latency", unavailable, "无已核验逐次耗时字段"),
        ] {
            fields.insert(
                key.into(),
                serde_json::json!({"availability": availability, "note":note}),
            );
        }
        CapabilityTable {
            adapter_id: "openclaw".into(),
            product: "OpenClaw".into(),
            surfaces: vec!["cli".into(), "gateway".into()],
            supported_versions: vec![],
            fields,
            discovery: serde_json::json!({"default_roots":["<home>/.openclaw"],"legacy_home":"仅默认新根不存在时 .clawdbot","env_override":["OPENCLAW_STATE_DIR","OPENCLAW_HOME"],"manual_roots":true,"bounded":true,"pattern":"agents/<agentId>/agent/openclaw-agent.sqlite；旧 sessions 仅诊断"}),
            detection: serde_json::json!({"magic":"schema 24、所需表列、schema_meta.role/agent_id 与路径一致","version_field":"整库 app_version 不认证历史行","registry":"adapters/openclaw/versions","fail_closed":true,"unknown_version":"schema 24 latest_fallback；其他 schema/迁移输入拒绝"}),
            lifecycle: serde_json::json!({"runtime_store":"hot transcript 正文独立读取；会话可变累计不叠加","legacy_archive":"迁移输入不读用量","cold_archive":"待非空真实样本；存在时显式覆盖缺口，保留既有历史"}),
            incremental: serde_json::json!({"cursor":"session_id/seq 分页，末页重查历史，offset=0","row_cap":runtime_store::MAX_ROWS_PER_ROUND,"body_cap":runtime_store::MAX_BODY_BYTES,"snapshot":"只读 SQLite；schema/来源/冷归档/正文同一快照"}),
            dedup: serde_json::json!({"primary":"session_id+entry id","rescan":"统一事件身份幂等；消失条目不删除已有历史","cross_source":"provenance=1，排除 ACP/plugin/hook/其他 harness 与迁移镜像"}),
            integrity: serde_json::json!({"success_only":false,"hidden_calls":"未落盘调用不补造；其他 transport/辅助路径待验","sampling":"未知","source_retention":"冷归档缺口可见"}),
            maintenance: serde_json::json!({"parser_version":runtime_store::OPENCLAW_PARSER_VERSION,"format_evidence":"官方 npm 2026.9.8 实装、schema 24、真实 CLI/续会话与独立 API","evidence_level":"real-local-cli","upgrade_policy":"每个 transport/历史版本与冷归档单独核验"}),
            scheduling: serde_json::json!({"entry":"统一 run_adapter_scan","incremental_cost":"有界分页/复查","pause_cancel":"SQLite VM、逐行与 zstd 解码协作检查"}),
            limitations: vec![
                "CLI 两次真实调用不认证 Gateway、辅助/嵌套/导入与所有历史版本".into(),
                "默认零和计算总量保持未知；调用数与费用不补造".into(),
                "冷归档与旧迁移输入仍待真实取证；其他 schema fail closed".into(),
                "npm provenance 源码与实际编译代码不同，按实装字段核验；整库版本不认证历史行"
                    .into(),
                "远端 Gateway 数据不属于本机来源".into(),
            ],
        }
    }
}
