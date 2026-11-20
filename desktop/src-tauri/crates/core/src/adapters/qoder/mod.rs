//! Qoder CLI transcripts from the official 1.1.76 persistence writer.
//! CLI-format request anchors identify transcripts with missing usage. Sealed session state adds no calls.

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
            roots.push((home.join(".qoder-cn/projects"), RootBasis::DefaultHome));
            roots.push((
                home.join(".lingma/vscode/sharedClientCache/cache/db"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out: Vec<DiscoveredRoot> = Vec::new();
        let mut add =
            |root: std::path::PathBuf, basis: RootBasis, files: Vec<std::path::PathBuf>| {
                if files.is_empty() {
                    return;
                }
                if let Some(existing) = out.iter_mut().find(|entry| entry.root == root) {
                    for file in files {
                        if !existing.files.contains(&file) {
                            existing.files.push(file);
                        }
                    }
                } else {
                    out.push(DiscoveredRoot { root, basis, files });
                }
            };
        for (root, basis) in roots {
            // Bounded discovery of session JSONL and sessions/<id>/state.json files.
            let accept = |p: &std::path::Path| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name == "state.json" || name.ends_with(".jsonl") || name == "local.db"
            };
            let mut files = if root.is_file() {
                vec![root.clone()]
            } else {
                crate::adapters::framework::enumerate_files_bounded(&root, 3, &accept)
            };
            for suffix in [
                "vscode/sharedClientCache/cache/db/local.db",
                "sharedClientCache/cache/db/local.db",
            ] {
                let path = root.join(suffix);
                if path.is_file() && !files.contains(&path) {
                    files.push(path);
                }
            }
            let mut json_files = Vec::new();
            for file in files {
                if versions::cn_vscode_db::is_sqlite(&file)
                    || file.file_name().is_some_and(|s| s == "local.db")
                {
                    let parent = file.parent().unwrap_or(&root).to_path_buf();
                    add(parent, basis.clone(), vec![file]);
                } else {
                    json_files.push(file);
                }
            }
            add(root, basis, json_files);
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
        if versions::cn_vscode_db::is_sqlite(&target.path) {
            versions::cn_vscode_db::scan(target, stored, limits, now_ms)
        } else if target.path.extension().is_some_and(|e| e == "jsonl") {
            versions::transcript_v1::scan(target, stored, limits, now_ms)
        } else {
            versions::probe_only::scan(target, stored, limits, now_ms)
        }
    }

    fn should_scan_unchanged(&self, stored: &crate::adapters::framework::StoredScanState) -> bool {
        probe_only::legacy_probe(stored)
    }

    fn scan_format(
        &self,
        outcome: &crate::adapters::framework::ScanOutcome,
    ) -> Option<crate::adapters::framework::DetectOutcome> {
        let context = outcome.parse_context.as_ref()?;
        use crate::domain::VersionBasis::{KnownVersion, LatestFallback};
        let (format, basis) = match context["sealed_state"].as_bool() {
            Some(true) => (probe_only::SEALED_STATE_FORMAT, KnownVersion),
            Some(false) => (QODER_FORMAT, LatestFallback),
            None if context["validated"] == true => {
                let format = if context["cn_database"] == true {
                    versions::cn_vscode_db::FORMAT
                } else {
                    QODER_FORMAT_VERSION
                };
                let basis = if context["fallback"] == true {
                    LatestFallback
                } else {
                    KnownVersion
                };
                (format, basis)
            }
            None => return None,
        };
        Some(crate::adapters::framework::DetectOutcome::Supported {
            format: format.into(),
            format_version: None,
            basis,
        })
    }

    fn has_validated_usage(&self, outcome: &crate::adapters::framework::ScanOutcome) -> bool {
        outcome.parse_context.as_ref().is_some_and(|context| {
            context["native_envelope"] == true || context["cn_database"] == true
        })
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
                serde_json::json!({"availability":Availability::Partial("覆盖范围随本地记录格式而异".into()),
                "note":"CLI 1.1.76 源码与构造数据；正值可读，默认/脱敏零未知。1.1.86-hotfix.5.3 本机样本可计已完成响应，token 未知。CN VS Code 2.6.11 仅对话观察，token/逐模型调用次数未知"}),
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
            product: "Qoder CLI / Qoder CN VS Code".into(),
            surfaces: vec!["cli".into(), "vscode".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .chain(std::iter::once("CN VS Code 2.6.11 / chat DB 2".into()))
                .collect(),
            discovery: serde_json::json!({"default_roots":["~/.qoder/projects","~/.qoder-cn/projects","~/.lingma/vscode/sharedClientCache/cache/db"],
                "env_override":"QODER_CONFIG_DIR", "manual_roots":"projects 目录、会话文件或 CN VS Code local.db/父目录", "bounded":true}),
            detection: serde_json::json!({"magic":"CLI assistant envelope / SQLite chat_session+chat_record+chat_message", "version_field":"CLI 每条记录 version；CN 会话 version 仅为格式版本",
                "registry":"adapters/qoder/versions", "unknown_version":"结构校验后兼容读取"}),
            fields,
            lifecycle: serde_json::json!({"partials":"stop_reason=null 不计", "fork":"跳过 forkedFrom 副本",
                "context":"token-stats/压缩/系统消息不计用量", "state":"已识别的加密会话状态不计用量，其他未验证结构保留诊断"}),
            incremental: serde_json::json!({"cursor":"CLI 完整 JSONL 行字节偏移；CN rowid 分页并周期重读 WAL 可变记录", "upgrade":"旧探针游标重读"}),
            dedup: serde_json::json!({"primary":"CLI sessionId + message.id；CN session_id + request_id"}),
            integrity: serde_json::json!({"default_zero":"unknown", "provider":"unknown",
                "timestamp":"持久化观察时间", "invalid_fields":"保留其他有效桶并报告诊断"}),
            maintenance: serde_json::json!({"parser_version":format!("{}+{}",versions::transcript_v1::PARSER_VERSION,versions::cn_vscode_db::PARSER),
                "format_evidence":"CLI 1.1.76 SessionTranscriptWriter；本机 1.1.86-hotfix.5.3 请求锚点及加密状态结构；CN VS Code 2.6.11 本地 SQLite 及插件请求构造",
                "evidence_level":"CLI 1.1.76 source-derived; 1.1.86-hotfix.5.3 native completion without tokens; CN VS Code local metadata observations"}),
            scheduling: serde_json::json!({"entry":"统一 run_adapter_scan"}),
            limitations: vec![
                "官方渠道脱敏 token 为 0，只计已完成响应次数，token 未知".into(),
                "CN VS Code 2.6.11 本地数据库只保留对话观察记录；token/金额/模型调用次数未知"
                    .into(),
                "本机 1.1.86-hotfix.5.3 的 CLI 格式响应未报告 token；独立 IDE/JetBrains 格式仍待核验".into(),
            ],
        }
    }
}
