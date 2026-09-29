//! jcode 适配器（jcode.sh，开源 Rust 终端 Agent；独立目录合同）。载体：
//! `$JCODE_HOME`（默认 ~/.jcode）/sessions/session_*.json 快照 +
//! 同 stem `.journal.jsonl`（journal 权威覆盖快照；按消息 id upsert 幂等）。

pub mod detect;
pub mod versions;

pub use detect::JCODE_FORMAT;
pub use versions::session_v1;
pub use versions::{JCODE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// jcode 适配器（无状态）。
pub struct JcodeAdapter;

impl Default for JcodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl JcodeAdapter {
    pub fn new() -> Self {
        JcodeAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for JcodeAdapter {
    fn adapter_id(&self) -> &'static str {
        "jcode"
    }

    fn agent(&self) -> &'static str {
        "jcode"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get("JCODE_HOME").filter(|v| !v.trim().is_empty()) {
            roots.push((
                std::path::PathBuf::from(home.trim()).join("sessions"),
                RootBasis::EnvOverride("JCODE_HOME".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".jcode").join("sessions"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // 只枚举快照；journal 由扫描层按 stem 组装（两载体一个逻辑单元）。
            let files = crate::adapters::framework::enumerate_files_bounded(&root, 1, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| {
                        n.starts_with("session_")
                            && n.ends_with(".json")
                            && !n.ends_with(".journal.jsonl")
                    })
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
            "jcode@{}",
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
        versions::session_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "官方源码证据（1jehuang/jcode 4f6bf8e）；本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "token_usage 逐 assistant 消息（input/output 必填，cache 读/写可选）；prompt_tokens 是上下文规模不采",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cache_read_input_tokens",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cache_creation_input_tokens",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每 assistant 消息一次（无独立 reasoning/cost 落盘）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "会话级 provider_key/model（journal meta 权威，最后一条为准）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "消息 timestamp（RFC3339 完成时刻）；tool_duration_ms → duration/interval_start",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("成本展示层自算不落盘（model_pricing.rs）".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Partial(note.clone()),
                "tool_duration_ms（工具回合时长，非纯模型延迟）",
            ),
        );
        CapabilityTable {
            adapter_id: "jcode".to_string(),
            product: "jcode（jcode.sh，开源 Rust）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["~/.jcode/sessions"],
                "env_override": "JCODE_HOME",
                "manual_roots": "sessions 目录",
                "bounded": true,
                "pattern": "session_<word>_<millis>_<hex>.json 快照（journal sidecar 由扫描层组装）；model-usage-v1.sqlite3 只记计数不读",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "JSON 含 id + messages（token_usage 可选）",
                "version_field": "无；文档级锚点 jcode-session-1",
                "registry": "adapters/jcode/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "snapshot_plus_journal": "官方加载语义：快照 + journal 回放；journal meta 覆盖快照元数据、append_messages 追加",
                "checkpoint": "checkpoint 整写快照并删除 journal（512 KiB 上限）⇒ 两载体消息互斥；崩溃窗口重复 extend 由本方按消息 id upsert 兜底（源码无去重）",
                "cache_semantics": "provider 原样保留：openai input 含 cache（uncached 派生）；anthropic 三列互斥；未知并列不派生",
            }),
            incremental: serde_json::json!({
                "cursor": "快照字节数 + journal 偏移；任一变化全量重读，事件按消息 id upsert 幂等",
            }),
            dedup: serde_json::json!({
                "primary": "jcode:<session>:<message id>",
                "crash_window": "快照尾部与 journal 头同 id：取 journal 版（后写权威）",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "缺 token_usage 的 assistant 消息 = 官方未知口径，不入账不补零",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::session_v1::JCODE_PARSER_VERSION,
                "format_evidence": "1jehuang/jcode 4f6bf8e（jcode-storage/jcode-base session 持久化 + provider openai/anthropic 流）",
                "evidence_level": "official-source（无本机样本）",
                "upgrade_policy": "上游 ^ 浮动依赖（锁文件版）：真实样本后复核字段",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "模型归因是会话级（journal meta 最后一条）：会话中换模型时逐消息模型不可考（StructuredChange）".into(),
                "无 cost/reasoning 落盘".into(),
                "本机未安装：文档级实现".into(),
            ],
        }
    }
}
