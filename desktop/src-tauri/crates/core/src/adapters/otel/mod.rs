//! OTel spans JSONL 适配器（M5 遥测载体；独立目录合同）。覆盖：
//! VS Code Copilot Chat file exporter（`github.copilot.chat.otel.*`，文档级）、
//! Copilot CLI file exporter（`COPILOT_OTEL_FILE_EXPORTER_PATH`，行级 schema
//! 未文档化——同族容错解析，待本机样本）、本应用 OTLP 接收器的归一化输出
//! （CodeBuddy agentlens 经接收器落盘）。
//! 汇总 span（invoke_agent/codebuddy_code.interaction）与 model_request
//! 按官方防双计警告跳过。

pub mod detect;
pub mod versions;

pub use detect::OTEL_FORMAT;
pub use versions::spans_doc1;
pub use versions::{LATEST_IMPL_ID, OTEL_FORMAT_VERSION, VERIFIED_VERSION_IMPLS};

/// OTel spans 适配器（无状态）。
pub struct OtelAdapter;

impl Default for OtelAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OtelAdapter {
    pub fn new() -> Self {
        OtelAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for OtelAdapter {
    fn adapter_id(&self) -> &'static str {
        "otel"
    }

    fn agent(&self) -> &'static str {
        "otel"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        // 本应用 OTLP 接收器的归一化输出（固定应用数据目录 otel/spans.jsonl）。
        if cfg!(windows) {
            if let Some(appdata) = ctx.env.get("APPDATA") {
                roots.push((
                    std::path::PathBuf::from(appdata)
                        .join("llm-usage-desktop")
                        .join("otel"),
                    RootBasis::DefaultHome,
                ));
            }
            if let Some(home) = &ctx.home_dir {
                roots.push((
                    home.join("AppData")
                        .join("Roaming")
                        .join("llm-usage-desktop")
                        .join("otel"),
                    RootBasis::DefaultHome,
                ));
            }
        } else if let Some(home) = &ctx.home_dir {
            let base = ctx
                .env
                .get("XDG_DATA_HOME")
                .filter(|v| !v.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            roots.push((
                base.join("llm-usage-desktop").join("otel"),
                RootBasis::DefaultHome,
            ));
        }
        // 用户启用外部 exporter 的文件（VS Code outfile / COPILOT_OTEL_FILE_EXPORTER_PATH）
        // 无固定默认路径：手工根添加。
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // 手工根可为目录或 *.jsonl 文件本身。
            let files = if root.is_file() {
                vec![root.clone()]
            } else {
                crate::adapters::framework::enumerate_files_bounded(&root, 2, &|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
                })
            };
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "otel@{}",
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
        versions::spans_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "官方文档证据（VS Code agent_monitoring.md bdc5ebe + Copilot CLI OTel 文档 + CodeBuddy agentlens 文档，2026-09-29）；需启用载体，本机无样本".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "chat/model_stream span 的 gen_ai.usage.*（Copilot/VS Code）或无前缀 usage.*（CodeBuddy）；包含关系未证不派生总量",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "gen_ai.usage.cache_read.input_tokens / usage.cache_read_input_tokens",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "gen_ai.usage.cache_creation.input_tokens / usage.cache_creation_input_tokens",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "每 chat/model_stream span 一次（汇总 span 与 model_request 按官方防双计跳过）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "gen_ai.request.model / model_name 等别名",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "span startTime（[秒,纳秒] 对或毫秒）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("nano_aiu/AIU 是计量单位非 USD：不映射".into()),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Partial(note.clone()),
                "TTFT 属性（copilot_chat.* ms / time_to_first_chunk 秒 / agentlens 容错折算）",
            ),
        );
        CapabilityTable {
            adapter_id: "otel".to_string(),
            product: "OTel 遥测载体（VS Code Copilot Chat file exporter / Copilot CLI file exporter / OTLP 接收器输出）".to_string(),
            surfaces: vec!["telemetry".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["%APPDATA%/llm-usage-desktop/otel（本应用 OTLP 接收器输出）"],
                "env_override": null,
                "manual_roots": "VS Code outfile / COPILOT_OTEL_FILE_EXPORTER_PATH 的文件或目录（需用户启用 exporter）",
                "bounded": true,
                "pattern": "spans JSONL（深度 ≤2）；Agent 归属按 resource service.name",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "gen_ai.* 命名空间或 span 记录形状（spanId/startTime）",
                "version_field": "无；文档级锚点 otel-spans-doc-1",
                "registry": "adapters/otel/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "不可用记录跳行诊断（键名双拼写容错已注明）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "no_double_count": "invoke_agent（全 turn 汇总）与 codebuddy_code.interaction/model_request 跳过——官方防双计警告",
                "per_request": "仅 chat（Copilot/VS Code）与 model_stream（CodeBuddy）入账",
            }),
            incremental: serde_json::json!({
                "cursor": "JSONL 字节偏移；事件键 otel:<spanId>",
            }),
            dedup: serde_json::json!({
                "primary": "otel:<spanId>",
                "cross_file": "同一 span 经接收器与外部文件双载体时按 spanId upsert 幂等",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "未启用 exporter/接收器的用量不可见（需启用载体，不回填）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::spans_doc1::OTEL_PARSER_VERSION,
                "format_evidence": "VS Code agent_monitoring.md bdc5ebe（file exporter 行格式与 chat span 属性）+ Copilot CLI OTel 文档 + CodeBuddy monitoring 文档",
                "evidence_level": "official-docs（需启用载体；Copilot CLI 行级 schema 待样本）",
                "upgrade_policy": "本机启用 exporter 取得真实样本后逐字段核验",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "需启用载体：VS Code/Copilot CLI exporter 或本应用 OTLP 接收器（均默认关闭）".into(),
                "Copilot CLI file exporter 行级 schema 未文档化：同族容错解析，待本机样本核验".into(),
                "trace/span ID 键名双拼写容错（文档未逐字给出）".into(),
                "agentlens TTFT 单位未标：>1e4 视为毫秒否则按秒折算（容错已注明）".into(),
            ],
        }
    }
}
