//! Xum 适配器（Coder，原 mux；独立目录约定）。载体：
//! `~/.mux/sessions/<workspaceId>/session-usage.json` 的 byModel 会话级
//! 按模型累计（IntervalAggregate；产品更名保留 mux 旧根）。

pub mod detect;
pub mod versions;

pub use detect::XUM_FORMAT;
pub use versions::usage_v1;
pub use versions::{LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS, XUM_FORMAT_VERSION};

/// Xum 适配器（无状态）。
pub struct XumAdapter;

impl Default for XumAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl XumAdapter {
    pub fn new() -> Self {
        XumAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for XumAdapter {
    fn adapter_id(&self) -> &'static str {
        "xum"
    }

    fn agent(&self) -> &'static str {
        "xum"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        for (canonical, legacy) in [
            ("XUM_ROOT", "MUX_ROOT"),
            ("XUM_RUN_SESSION_ROOT", "MUX_RUN_SESSION_ROOT"),
        ] {
            if let Some(value) = ctx
                .env
                .get(canonical)
                .or_else(|| ctx.env.get(legacy))
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                let name = if ctx.env.contains_key(canonical) {
                    canonical
                } else {
                    legacy
                };
                roots.push((
                    std::path::PathBuf::from(value).join("sessions"),
                    RootBasis::EnvOverride(name.into()),
                ));
            }
        }
        // 旧品牌 mux 根保留（产品更名；第三方解析器读取的路径即 ~/.mux）。
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".mux").join("sessions"), RootBasis::DefaultHome));
            roots.push((home.join(".xum").join("sessions"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == "session-usage.json")
                    .unwrap_or(false)
            });
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "xum@{}",
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
        versions::usage_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "官方 npm 0.30.0 / coder/xum 81b0b744 与默认/网关对照真实本地模型载体；其他场景待核验"
                .to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "byModel 累计：正 input 为未缓存输入；零桶未知；文本输出加已知推理，推理未知时是下界；完整输入/总量未知",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "正 cached.tokens；默认零未知",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "正 cacheCreate.tokens；默认零未知",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Unavailable("仅会话级聚合，无逐请求数据（不虚构逐次）".into()),
                "无",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "byModel 键 provider:model（split 拆分）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "lastRequest.timestamp（毫秒；缺失回退 mtime）为区间端点；起始未知",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable(
                    "会话级聚合无逐事件 cost 载体（与 hermes 同型限制）；cost_usd 字段存在但不映射"
                        .into(),
                ),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无逐次时间".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "xum".to_string(),
            product: "Xum（Coder，原 mux）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.mux/sessions（旧品牌根）", "~/.xum/sessions（官方新根）"],
                "env_override": "XUM_ROOT/MUX_ROOT、XUM_RUN_SESSION_ROOT/MUX_RUN_SESSION_ROOT 下的 sessions；CLI 默认临时目录退出删除",
                "manual_roots": "sessions 目录或 workspace 目录",
                "bounded": true,
                "pattern": "sessions/<workspaceId>/session-usage.json（深度 2）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "文件含 byModel + lastRequest/version 指纹",
                "version_field": "version 字段是内容版本整数；文档级锚点 xum-session-usage-doc-1",
                "registry": "adapters/xum/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "aggregate_only": "每 workspace×模型一条 session 级聚合；不展开伪造逐次",
                "cumulative": "byModel 值为会话累计（重写文件，upsert 幂等）",
            }),
            incremental: serde_json::json!({
                "cursor": "整写 JSON：已消费字节数；改写走 generation 重扫",
                "idempotent": "scope_key=xum:<workspace>:<provider:model>，source_revision=端点毫秒",
            }),
            dedup: serde_json::json!({ "primary": "xum:<workspaceId>:<model_key>" }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "仅 product 自记的会话累计；逐次明细不可见",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::usage_v1::XUM_PARSER_VERSION,
                "format_evidence": "官方 npm 0.30.0 与对应提交 81b0b744 的 sessionUsageService/displayUsage；两次真实本地模型载体",
                "evidence_level": "real-local（默认缺 usage 的失败任务与网关仅请求真实 usage 的成功对照分别保留）",
                "upgrade_policy": "完整旧摘要仅允许 input 桶纠正、默认零和已知推理归并；旧游标自动重评，不认证其他产品版本",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "会话级聚合：无逐请求数据/延迟；区间起始未知（end=lastRequest.timestamp/mtime）"
                    .into(),
                "cost_usd 不映射（聚合层无 cost 载体；与 hermes 同型限制）".into(),
                "默认 custom OpenAI-compatible 不请求流式 usage，真实缺 usage 写入五零；网关对照仅请求实际模型 usage，响应原样转发，不认证默认覆盖".into(),
                "零桶不能区分默认值；正文本输出在推理未知时只作下界，完整输入/总 token 未知".into(),
                "CLI 默认删除临时会话；须保留 RUN_SESSION_ROOT 或手工根才能采集该次原生载体".into(),
            ],
        }
    }

    fn prior_aggregate_hashes(
        &self,
        input: &crate::aggregates::SourceAggregateInput,
    ) -> Vec<String> {
        versions::usage_v1::prior_display_hashes(input)
    }
}
