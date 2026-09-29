//! Aider 适配器（独立目录合同 architecture.md#adapter-layout）：
//! - 载体是 `--analytics-log <file>` 的本地 JSONL（需启用，不回填历史）；
//! - 默认**无固定路径**（args.py 无 default）：发现只走手工根——
//!   用户把 analytics 日志文件本身或其所在目录加为手工根；
//! - `.aider.chat.history.md`/`.aider.input.history`/`--llm-history-file`
//!   均无逐次 usage（固定源码 5dc9490 核验），不采集。
//!
//! 版本注册表唯一条目：文档级 aider-analytics-doc-1。

pub mod detect;
pub mod versions;

pub use detect::AIDER_FORMAT;
pub use versions::analytics_doc1;
pub use versions::{AIDER_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

/// Aider 适配器（无状态）。
pub struct AiderAdapter;

impl Default for AiderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AiderAdapter {
    pub fn new() -> Self {
        AiderAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for AiderAdapter {
    fn adapter_id(&self) -> &'static str {
        "aider"
    }

    fn agent(&self) -> &'static str {
        "aider"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        // 固定源码无默认 analytics 路径：只接受手工根（文件或目录）。
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for manual in &ctx.manual_roots {
            let files = if manual.is_file() {
                vec![manual.clone()]
            } else if manual.is_dir() {
                // 目录：枚举一层 *.jsonl（探测指纹会过滤非 analytics 文件）。
                crate::adapters::framework::enumerate_files_bounded(manual, 1, &|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
                })
            } else {
                continue;
            };
            let root = manual
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| manual.clone());
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot {
                    root,
                    basis: RootBasis::Manual,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "aider@{}",
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
        versions::analytics_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "官方源码证据（5dc9490）；本机未安装（2026-09-29 盘点），且需用户显式启用 --analytics-log，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "message_send 事件 prompt_tokens/completion_tokens/total_tokens（逐次）；prompt 含 cache 写（官方并入口径）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Unavailable(
                    "cache_hit 只进成本公式不上报（base_coder.py:2087-2100）".into(),
                ),
                "无",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Unavailable("cache_creation 并入 prompt_tokens，无独立字段".into()),
                "已并入 input",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每条 message_send 一次发送汇总（/tokens 报告后触发）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "properties.main_model；models.db 未知模型可能被脱敏为 provider/REDACTED（按原文入账）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "time 字段 Unix 秒（事件写入时刻）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(note.clone()),
                "properties.cost（本条，litellm 费率自算 ⇒ Estimated，micro-USD）；total_cost 是会话累计不入账",
            ),
        );
        fields.insert(
            "latency".into(),
            field(Availability::Unavailable("无延迟字段".into()), "无"),
        );
        CapabilityTable {
            adapter_id: "aider".to_string(),
            product: "Aider（Aider-AI，--analytics-log 载体）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [],
                "env_override": null,
                "manual_roots": "analytics 日志文件本身或其所在目录（默认无固定路径，须用户显式指定 --analytics-log）",
                "bounded": true,
                "pattern": "JSONL 追加式；指纹 = 首行 event/properties/time",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSON 对象含 event/properties/time",
                "version_field": "无；文档级锚点 aider-analytics-doc-1",
                "registry": "adapters/aider/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed，不走版本回退",
            }),
            fields,
            lifecycle: serde_json::json!({
                "carrier": "message_send 在 /tokens 报告输出后、计数器清零前写入（base_coder.py:2102-2126）：一次发送一次事件",
                "no_backfill": "启用前历史不可回填（官方无回填路径）；仅自启用时刻起累积",
                "other_events": "launched/exit/command_* 等事件无 usage，跳过",
            }),
            incremental: serde_json::json!({
                "cursor": "JSONL 字节偏移（追加式）；事件键 = 整行内容哈希，重放幂等",
            }),
            dedup: serde_json::json!({
                "primary": "aider:<整行内容哈希>",
                "same_file": "同秒多条 message_send 靠内容哈希区分（无消息 ID）",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "未启用 --analytics-log 的会话完全不可见；message_send_exception 事件无 usage 不入账",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::analytics_doc1::AIDER_PARSER_VERSION,
                "format_evidence": "Aider-AI/aider 固定源码 5dc9490（args.py/analytics.py/base_coder.py/io.py）+ 官方样本 sample-analytics.jsonl",
                "evidence_level": "official-source（无本机真实样本）",
                "upgrade_policy": "真实样本出现后按实际事件字段升级验证",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan",
                "incremental_cost": "追加式字节偏移增量",
            }),
            limitations: vec![
                "需启用载体：默认不开启 --analytics-log，启用前无任何数据（不回填）".into(),
                "无 cache 读写分项（prompt_tokens 含 cache 写；cache_hit 不上报）".into(),
                "模型名可能被上游脱敏（provider/REDACTED），按原文入账不还原".into(),
                "本机未安装：discover 仅在手工根下生效".into(),
            ],
        }
    }
}
