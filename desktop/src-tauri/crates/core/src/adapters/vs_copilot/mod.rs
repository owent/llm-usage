//! Built-in Visual Studio GitHub Copilot telemetry adapter. The local JSONL path is
//! <temporary directory>/VSGitHubCopilotLogs/traces/<hex>_VSGitHubCopilot_traces.jsonl:
//! verified VS 18 components automatically write one OTLP resourceSpans batch per line.
//! This does not verify all VS 2022/older extensions; see m9-vs-copilot-discovery.md for version limits.
//!
//! Format references: read-only local checks on 2026-10-01 using VS 18 Community
//! 18.10.1197+4b9e241b86 and DevHub/.NET 10.0.12.
//! - resource.attributes includes service.name=vs-copilot, service.namespace=visualstudio,
//!   service.version and process.runtime.name=DevHub.
//! - chat <model> CLIENT spans (kind=3) represent individual LLM requests and contain
//!   gen_ai.usage.input_tokens/gen_ai.usage.output_tokens,
//!   gen_ai.usage.cache_read.input_tokens with string int64 values such as {intValue:"8697"},
//!   gen_ai.request.model/gen_ai.response.model,
//!   gen_ai.conversation.id (the VSGitHubCopilot session UUID),
//!   copilot_chat.root_request_id and server.address=api.githubcopilot.com.
//!   startTimeUnixNano/endTimeUnixNano are numeric/string nanosecond timestamps.
//! - invoke_agent GitHub Copilot root spans summarize whole turns without usage.
//!   Read chat spans and skip summaries, following the OTel duplicate-counting warning.
//! - gen_ai.input.messages/gen_ai.output.messages/gen_ai.tool.definitions contain
//!   conversation/tool content; read selected usage keys without storing or printing bodies.
//!
//! Temporary files can be deleted by users/system cleanup, and VS rolls telemetry files.
//! Source disappearance retains imported results through framework file-generation rules; history may be incomplete.

pub mod detect;
pub mod versions;

pub use detect::VS_COPILOT_FORMAT;

/// Stateless VS Copilot adapter.
pub struct VsCopilotAdapter;

impl Default for VsCopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VsCopilotAdapter {
    pub fn new() -> VsCopilotAdapter {
        VsCopilotAdapter
    }
}

/// FileOutputResolver uses Path.GetTempPath(); Windows resolves TMP before TEMP.
/// Collect both variables and the observed default user temporary directory without SKU/year filters.
/// Use only the supplied context so manual_roots_only/isolated tests cannot read host sources.
fn default_traces_dirs(
    ctx: &crate::adapters::framework::DiscoverContext,
) -> Vec<std::path::PathBuf> {
    let env_path = |key: &str| {
        ctx.env
            .get(key)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from)
    };
    let mut temps: Vec<_> = [env_path("TMP"), env_path("TEMP")]
        .into_iter()
        .flatten()
        .collect();
    // Current-user GetTempPath fallback; do not enumerate Windows/SystemTemp or other users.
    if temps.is_empty() {
        if let Some(profile) = env_path("USERPROFILE").or_else(|| ctx.home_dir.clone()) {
            temps.push(profile);
        }
    }
    if let Some(local) = env_path("LOCALAPPDATA") {
        temps.push(local.join("Temp"));
    }
    temps
        .into_iter()
        .map(|temp| temp.join("VSGitHubCopilotLogs").join("traces"))
        .collect()
}

impl crate::adapters::framework::SourceAdapter for VsCopilotAdapter {
    fn adapter_id(&self) -> &'static str {
        "vs-copilot"
    }

    /// Agent name matches resource service.name="vs-copilot".
    /// The otel adapter uses the same name, grouping both formats under the same Agent.
    fn agent(&self) -> &'static str {
        "vs-copilot"
    }

    fn should_scan_unchanged(&self, stored: &crate::adapters::framework::StoredScanState) -> bool {
        versions::traces_v1::should_scan_unchanged(stored)
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        for dir in default_traces_dirs(ctx) {
            if dir.is_dir() {
                roots.push((dir, RootBasis::DefaultHome));
            }
        }
        for manual in &ctx.manual_roots {
            if manual.is_file() && manual.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                if let Some(parent) = manual.parent() {
                    roots.push((parent.to_path_buf(), RootBasis::Manual));
                }
                continue;
            }
            if !manual.is_dir() {
                continue;
            }
            // Manual roots accept traces, its VSGitHubCopilotLogs parent or the containing temporary directory.
            if manual
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("traces"))
            {
                roots.push((manual.clone(), RootBasis::Manual));
            } else if manual.join("traces").is_dir() {
                roots.push((manual.join("traces"), RootBasis::Manual));
            } else if manual.join("VSGitHubCopilotLogs").join("traces").is_dir() {
                roots.push((
                    manual.join("VSGitHubCopilotLogs").join("traces"),
                    RootBasis::Manual,
                ));
            }
        }
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (dir, basis) in roots {
            let mut files = crate::adapters::framework::enumerate_files_bounded(&dir, 0, &|p| {
                p.extension().and_then(|e| e.to_str()) == Some("jsonl")
            });
            if basis == RootBasis::Manual
                && !ctx
                    .manual_roots
                    .iter()
                    .any(|p| p.is_dir() && dir.starts_with(p))
            {
                files.retain(|p| ctx.manual_roots.iter().any(|m| m == p));
            }
            if files.is_empty() {
                continue;
            }
            let key = crate::adapters::framework::normalize_path(&dir);
            let key = if cfg!(windows) {
                key.to_lowercase()
            } else {
                key
            };
            if seen.insert(key) {
                out.push(DiscoveredRoot {
                    root: dir,
                    basis,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "vs-copilot@{}",
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
        versions::traces_v1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Available,
                "gen_ai.usage.input_tokens/output_tokens（逐请求 span）",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Available,
                "gen_ai.usage.cache_read.input_tokens",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(
                    "键 gen_ai.usage.cache_creation.input_tokens 本机样本未出现，解析容错支持"
                        .into(),
                ),
                "出现即采",
            ),
        );
        fields.insert(
            "reasoning".into(),
            field(
                Availability::Partial(
                    "键 gen_ai.usage.reasoning.output_tokens 本机样本未出现，解析容错支持".into(),
                ),
                "出现即采",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "每 LLM 请求一个 chat span（真实模型请求数，优于 turn 粒度）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "gen_ai.response.model/request.model（如 gpt-5.3-codex）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "endTimeUnixNano（完成时刻；纳秒，数字或字符串形）",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("遥测无费用属性：不映射".into()),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Available,
                "endTime−startTime（派生毫秒）；TTFT 键本机样本未出现，容错支持",
            ),
        );
        CapabilityTable {
            adapter_id: "vs-copilot".to_string(),
            product: "GitHub Copilot for Visual Studio".to_string(),
            surfaces: vec!["visual-studio".into()],
            supported_versions: vec!["1".to_string()],
            discovery: serde_json::json!({
                "default_roots": ["%TMP%/VSGitHubCopilotLogs/traces", "%TEMP%/VSGitHubCopilotLogs/traces", "%LOCALAPPDATA%/Temp/VSGitHubCopilotLogs/traces (win)"],
                "env_override": "TMP/TEMP 均检查；两者缺失时检查当前 USERPROFILE；按物理目录去重",
                "manual_roots": "traces 目录 / VSGitHubCopilotLogs 目录 / 临时目录 / 单个 .jsonl 遥测文件",
                "bounded": true,
                "pattern": "单目录 *.jsonl 枚举（每 VS 实例一个 <hex>_VSGitHubCopilot_traces.jsonl）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSON resourceSpans[] 且 resource.attributes service.name=vs-copilot",
                "version_field": "service.version（资源属性，仅记录）；结构锚定 v1",
                "registry": "adapters/vs_copilot/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "信封同形但 service 不同记 UnknownFormat（非本产品遥测）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_request": "逐行 resourceSpans 批次 → 逐 span；chat span=一次模型请求（Final）",
                "storage_migration": "TEMP 清理/VS 实例滚动：来源消失保留旧结果，不回退双计",
            }),
            incremental: serde_json::json!({
                "cursor": "标准 JSONL 字节偏移（批次行追加写）",
            }),
            dedup: serde_json::json!({
                "primary": "vs-copilot:span:<traceId>:<spanId>",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "有完整 trace/span 身份及时间的 chat span 即计调用；无 usage 保留未知；inline 不经遥测时不计",
                "privacy": "gen_ai.input.messages/tool.definitions 含提示正文：解析只读白名单键，正文不入库不输出",
                "no_double_count": "invoke_agent 汇总 span 跳过（官方 OTel 防双计警告同族规则）",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::traces_v1::VS_COPILOT_PARSER_VERSION,
                "format_evidence": "本机 VS 18 Community 真实数据只读取证（2026-10-01：resource/span 属性、intValue 字符串形、纳秒时间戳、2 chat + 2 invoke_agent span）",
                "installation_discovery": "遥测发现独立于安装目录和 SKU；安装核查使用 desktop/scripts/inspect-vs-copilot.ps1 的 vswhere 全实例查询",
                "evidence_level": "real-data（本机全字段核对）",
                "upgrade_policy": "属性集/信封变化走 latest 兼容尝试，真实样本后锚定新版本",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "不按 Community/Professional/Enterprise 或安装年份筛选；仅采实际存在且通过格式核验的遥测。已核查的 VS 2022 Copilot 17.14.1713.63837 没有 VS 18 的 JSONL exporter，不能以安装完成认证 token 覆盖".into(),
                "载体在 TEMP：系统清理/磁盘清理会删除历史遥测，覆盖随 VS 实例滚动（不承诺完整历史）".into(),
                "invoke_agent 整 turn 汇总 span 跳过防双计；只用 chat span 逐请求计数".into(),
                "与 otel 适配器边界：把 VS span 经接收器再导出给 otel 载体会重复计数（同 agent 维度 vs-copilot）；两者取其一".into(),
                "cache_write/reasoning/TTFT 属性本机样本未出现：解析容错支持，出现即采".into(),
                "token 包含关系（input 是否含 cache_read）未由 VS 文档声明：并列报告不派生 uncached（与 otel 家族口径一致）".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::framework::{DiscoverContext, RootBasis, SourceAdapter};

    /// Default discovery builds VSGitHubCopilotLogs/traces under TEMP and enumerates JSONL files.
    #[test]
    fn default_discovery_finds_traces_dir() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-vs-copilot-discover-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let traces = dir.join("VSGitHubCopilotLogs").join("traces");
        std::fs::create_dir_all(&traces).unwrap();
        std::fs::write(traces.join("abc_VSGitHubCopilot_traces.jsonl"), "{}\n").unwrap();
        std::fs::write(dir.join("VSGitHubCopilotLogs").join("chat.log"), "log").unwrap();
        let mut env = std::collections::BTreeMap::new();
        env.insert("TEMP".to_string(), dir.to_string_lossy().to_string());
        let roots = VsCopilotAdapter::new().discover(&DiscoverContext {
            home_dir: None,
            env,
            manual_roots: Vec::new(),
        });
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].files.len(), 1);
        assert!(matches!(roots[0].basis, RootBasis::DefaultHome));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Manual roots accept traces, the VSGitHubCopilotLogs parent or an individual file.
    #[test]
    fn manual_root_shapes() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-vs-copilot-manual-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let logs = dir.join("VSGitHubCopilotLogs");
        let traces = logs.join("traces");
        std::fs::create_dir_all(&traces).unwrap();
        std::fs::write(traces.join("t.jsonl"), "{}\n").unwrap();
        let adapter = VsCopilotAdapter::new();
        for manual in [traces.clone(), logs.clone(), traces.join("t.jsonl")] {
            let roots = adapter.discover(&DiscoverContext {
                home_dir: None,
                env: Default::default(),
                manual_roots: vec![manual],
            });
            assert_eq!(roots.len(), 1, "roots={roots:?}");
            assert_eq!(roots[0].files.len(), 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
