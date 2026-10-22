//! Qoder CLI transcripts from the official 1.1.76 persistence writer.
//! IDE/JetBrains and encrypted state remain unverified; source fixtures are not native acceptance.

pub mod detect;
pub mod versions;

pub use detect::QODER_FORMAT;
pub use versions::probe_only;
pub use versions::{LATEST_IMPL_ID, QODER_FORMAT_VERSION, VERIFIED_VERSION_IMPLS};

/// Stateless Qoder adapter.
pub struct QoderAdapter;

impl Default for QoderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl QoderAdapter {
    pub fn new() -> Self {
        QoderAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for QoderAdapter {
    fn adapter_id(&self) -> &'static str {
        "qoder"
    }

    fn agent(&self) -> &'static str {
        "qoder"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx
            .env
            .get("QODER_CONFIG_DIR")
            .filter(|v| !v.trim().is_empty())
        {
            roots.push((
                std::path::PathBuf::from(dir.trim()).join("projects"),
                RootBasis::EnvOverride("QODER_CONFIG_DIR".to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".qoder").join("projects"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // Bounded discovery of session JSONL and sessions/<id>/state.json files.
            let accept = |p: &std::path::Path| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == "state.json" || name.ends_with(".jsonl")
            };
            let files = if root.is_file() && accept(&root) {
                vec![root.clone()]
            } else {
                crate::adapters::framework::enumerate_files_bounded(&root, 3, &accept)
            };
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "qoder@{}",
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
        if target.path.extension().is_some_and(|e| e == "jsonl") {
            versions::transcript_v1::scan(target, stored, limits, now_ms)
        } else {
            versions::probe_only::scan(target, stored, limits, now_ms)
        }
    }

    fn scan_format(
        &self,
        outcome: &crate::adapters::framework::ScanOutcome,
    ) -> Option<crate::adapters::framework::DetectOutcome> {
        let context = outcome.parse_context.as_ref()?;
        if context["validated"] != true {
            return None;
        }
        Some(crate::adapters::framework::DetectOutcome::Supported {
            format: QODER_FORMAT_VERSION.into(),
            format_version: None,
            basis: if context["fallback"] == true {
                crate::domain::VersionBasis::LatestFallback
            } else {
                crate::domain::VersionBasis::KnownVersion
            },
        })
    }

    fn has_validated_usage(&self, outcome: &crate::adapters::framework::ScanOutcome) -> bool {
        outcome
            .parse_context
            .as_ref()
            .is_some_and(|context| context["native_envelope"] == true)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let mut fields = serde_json::Map::new();
        for key in [
            "tokens",
            "cache_read",
            "cache_write",
            "per_request_calls",
            "model",
            "time",
        ] {
            fields.insert(
                key.into(),
                serde_json::json!({"availability":Availability::Partial("源码验证；实机未验".into()),
                "note":"CLI 1.1.76 持久化源码及构造数据；正值可读，默认/脱敏零未知；实机未验"}),
            );
        }
        for key in ["cost", "latency"] {
            fields.insert(
                key.into(),
                serde_json::json!({"availability":Availability::Unavailable("字段未证实".into()),
                "note":"Credits 不转换为金额；未证实本地耗时"}),
            );
        }
        CapabilityTable {
            adapter_id: "qoder".into(),
            product: "Qoder CLI".into(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({"default_roots":["~/.qoder/projects"],
                "env_override":"QODER_CONFIG_DIR", "manual_roots":"projects 目录或会话文件", "bounded":true}),
            detection: serde_json::json!({"magic":"CLI assistant envelope", "version_field":"每条记录 version",
                "registry":"adapters/qoder/versions", "unknown_version":"结构校验后兼容读取"}),
            fields,
            lifecycle: serde_json::json!({"partials":"stop_reason=null 不计", "fork":"跳过 forkedFrom 副本",
                "context":"token-stats/压缩/系统消息不计用量", "state":"state.json 保留未验证诊断"}),
            incremental: serde_json::json!({"cursor":"完整 JSONL 行字节偏移", "upgrade":"旧探针游标重读"}),
            dedup: serde_json::json!({"primary":"sessionId + message.id"}),
            integrity: serde_json::json!({"default_zero":"unknown", "provider":"unknown",
                "timestamp":"持久化观察时间", "invalid_fields":"保留其他有效桶并报告诊断"}),
            maintenance: serde_json::json!({"parser_version":versions::transcript_v1::PARSER_VERSION,
                "format_evidence":"@qoder-ai/qodercli 1.1.76 KLo/bxu/kU/cU/SessionTranscriptWriter",
                "evidence_level":"source-derived / synthetic-regression / native-pending"}),
            scheduling: serde_json::json!({"entry":"统一 run_adapter_scan"}),
            limitations: vec![
                "官方渠道脱敏 token 为 0，只计已完成响应次数，token 未知".into(),
                "IDE/JetBrains 独立格式尚未证实；不能沿用 CLI 字段".into(),
                "源码构造数据已验证；未安装客户端，实机验收待完成".into(),
            ],
        }
    }
}
