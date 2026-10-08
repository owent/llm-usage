//! OTel spans JSONL and verified Qwen SDK files (M5); independent adapter directory. Coverage:
//! VS Code Copilot Chat file exporter, github.copilot.chat.otel.*, native checked 2026-10-02;
//! Copilot CLI file exporter, COPILOT_OTEL_FILE_EXPORTER_PATH, whose line schema remains
//! undocumented: try compatible span reading, with native samples pending. JetBrains Copilot file export
//! uses otelExporterType/otelOutfile; fixed-source checks are recorded in
//! docs/validation/desktop-usage/m9-jb-copilot-analysis.md, 2026-10-01.
//! Also read normalized local receiver output, including CodeBuddy agentlens files.
//! Skip aggregate invoke_agent/codebuddy_code.interaction and model_request spans
//! under the official warning against double counting.

pub mod detect;
use versions::qwen_sdk_025 as qwen_sdk;
mod sdk_json;
pub mod versions;

pub use detect::OTEL_FORMAT;
pub use versions::spans_doc1;
pub use versions::{LATEST_IMPL_ID, OTEL_FORMAT_VERSION, VERIFIED_VERSION_IMPLS};

/// Stateless OTel spans adapter.
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
        // Normalized local receiver output under application data: otel/spans.jsonl.
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
        // External exporter files: VS Code outfile / COPILOT_OTEL_FILE_EXPORTER_PATH.
        // They have no fixed default path; add them through manual roots.
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        let mut seen_files = std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // Manual roots may be directories or individual files; directory discovery reads JSON/JSONL.
            let mut files = if root.is_file() {
                vec![root.clone()]
            } else {
                crate::adapters::framework::enumerate_files_bounded(&root, 2, &|p| {
                    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        e.eq_ignore_ascii_case("jsonl") || e.eq_ignore_ascii_case("json")
                    })
                })
            };
            files.retain(|p| seen_files.insert(crate::adapters::framework::normalize_path(p)));
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
        let qwen_sdk = (!target.rescan
            && stored
                .parse_context
                .as_ref()
                .is_some_and(|v| v["qwen_sdk_file"] == true))
            || crate::adapters::framework::read_detect_head(&target.path, 64 * 1024)?
                .is_some_and(|head| qwen_sdk::detect_head(&head).is_some());
        if qwen_sdk {
            return qwen_sdk::scan(target, stored, limits, now_ms);
        }
        versions::spans_doc1::scan(target, stored, limits, now_ms)
    }

    fn should_scan_unchanged(&self, stored: &crate::adapters::framework::StoredScanState) -> bool {
        if let Some(context) = stored
            .parse_context
            .as_ref()
            .filter(|v| v["qwen_sdk_file"] == true)
        {
            return context["policy_version"] != 1;
        }
        versions::spans_doc1::should_scan_unchanged(stored)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note = "VS Code 1.140.0 file 本机 30 个 CLIENT span 已核验（2026-10-02）；Qwen 0.25.0 SDK file 主/后台 2 次/16,423 token 已核对；CLI/JetBrains/CodeBuddy 分版本另验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "chat/model_stream 的 gen_ai.usage.* 或 usage.*；Qwen 0.25.0 SDK 按固定 input/output/reasoning 属性，reasoning 为 0 时派生总量；其他包含关系未知",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "gen_ai.usage.cache_read.input_tokens / usage.cache_read_input_tokens；Qwen SDK 为 gen_ai.usage.cached_input_tokens",
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
                "每已核验 chat/model_stream/Qwen llm_request span 一次；日志、HTTP span、指标与汇总 span 不叠加",
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
            product: "OTel 遥测载体（VS Code/Copilot CLI/JetBrains file 导出、Qwen Code SDK file、OTLP 接收器输出）".to_string(),
            surfaces: vec!["telemetry".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "%APPDATA%/llm-usage-desktop/otel（本应用 OTLP 接收器输出，Windows）",
                    "$XDG_DATA_HOME|~/.local/share/llm-usage-desktop/otel（Linux/macOS）"
                ],
                "env_override": null,
                "manual_roots": "VS Code outfile / COPILOT_OTEL_FILE_EXPORTER_PATH / JetBrains otelOutfile / Qwen telemetry.outfile；本应用管理的已核验 Qwen 输出自动定向发现",
                "bounded": true,
                "pattern": "spans JSONL 或 Qwen SDK 连续 JSON（深度 ≤2）；Agent 归属按 resource service.name/version",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "gen_ai.* span 或 resource._rawAttributes 的 qwen-code SDK 对象",
                "version_field": "通用文档锚点 otel-spans-doc-1；Qwen SDK service.version 逐对象限定 0.25.0",
                "registry": "adapters/otel/versions 的通用文档与 Qwen SDK 0.25.0 独立条目",
                "fail_closed": true,
                "unknown_version": "不可用记录跳行诊断（键名双拼写容错已注明）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "no_double_count": "invoke_agent（全 turn 汇总）与 codebuddy_code.interaction/model_request 跳过——官方防双计警告",
                "per_request": "chat（Copilot/VS Code）、model_stream（CodeBuddy）或已核验 Qwen 0.25.0 SDK INTERNAL llm_request；其他 Qwen 版本隔离",
            }),
            incremental: serde_json::json!({
                "cursor": "JSONL 字节偏移；Qwen SDK 文件按完整 JSON 对象边界，半对象/上限保持续读位置",
            }),
            dedup: serde_json::json!({
                "primary": "通用 otel:<spanId>；Qwen SDK 按 trace+span 元组",
                "cross_file": "已核验 trace+span 在相同主机/用户内择一；Qwen SDK 与原生按会话/本地日择一，保留原生与封存分区",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "未启用 exporter/接收器的用量不可见（需启用载体，不回填）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::spans_doc1::OTEL_PARSER_VERSION,
                "sdk_parser_version": qwen_sdk::PARSER,
                "format_evidence": "VS Code agent_monitoring.md bdc5ebe（file exporter 行格式与 chat span 属性）+ Copilot CLI OTel 文档 + CodeBuddy monitoring 文档 + JetBrains 插件字节码取证（OTelSpanProvider 解析 gen_ai.usage 五桶，2026-10-01）",
                "evidence_level": "VS Code 本机真实 file、Qwen 0.25.0 官方固定源码与真实主/后台 SDK file；Copilot CLI/JetBrains 行级 schema 待样本",
                "upgrade_policy": "本机启用 exporter 取得真实样本后逐字段核验",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "需启用载体：VS Code/Copilot CLI exporter 或本应用 OTLP 接收器（均默认关闭）".into(),
                "Copilot CLI file exporter 行级 schema 未文档化：同族容错解析，待本机样本核验".into(),
                "JetBrains 面同族（插件 1.18.0-261 源码级取证 2026-10-01）：默认本地仅 Nitrite 会话库的 turnCredits（credit 非 token）与 idea.log；逐次 token 需在插件设置启用 otelExporterType=file + otelOutfile 后把 outfile 加为手工根，行格式同族容错待真实样本锚定；otelCaptureContent 启用时 outfile 含提示正文，本应用只读白名单键".into(),
                "trace/span ID 键名双拼写容错（文档未逐字给出）".into(),
                "agentlens TTFT 单位未标：>1e4 视为毫秒否则按秒折算（容错已注明）".into(),
            ],
        }
    }
}
