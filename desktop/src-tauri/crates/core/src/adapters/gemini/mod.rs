//! Gemini CLI adapter directory: architecture.md#adapter-layout.
//! - Stable agent entry point with interface implementation and reexports.
//! - detect checks the documented format; no native version field for dispatch.
//! - versions exposes the single documented session-doc-1 format.
//! - Shared gemini/qwen map_genai_usage remains in root usage_map.rs.
//!
//! See versions::session_doc1 for format references.
//! M2/V30 moved root gemini.rs without changing rejection behavior:
//! reject undocumented message types or tokens outside the expected records.

pub mod detect;
pub mod versions;

pub use detect::GEMINI_FORMAT;
pub use session_doc1::{GEMINI_MAX_FILE_BYTES, GEMINI_PARSER_VERSION};
pub use versions::{session_doc1, GEMINI_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Remove UTF-8 BOM for detection/parsing; return BOM-free input unchanged.
fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes)
}

/// Stateless Gemini CLI adapter.
pub struct GeminiAdapter;

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiAdapter {
    pub fn new() -> Self {
        GeminiAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for GeminiAdapter {
    fn adapter_id(&self) -> &'static str {
        "gemini"
    }

    fn agent(&self) -> &'static str {
        "gemini-cli"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".gemini"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let tmp = root.join("tmp");
            if !tmp.is_dir() {
                continue;
            }
            // Bounded depth-two JSON enumeration under tmp; expected chats use tmp/<project_hash>/chats/session-*.json.
            let files = crate::adapters::framework::enumerate_files_bounded(&tmp, 2, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".json"))
                    .unwrap_or(false)
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "gemini@{}",
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
        // One format without version dispatch. If the registry later adds implementations
        // such as session-doc-2, dispatch here using the detection result.
        versions::session_doc1::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
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
                Availability::Partial(
                    "文档级证据，待真实样本：六键各自可选；thoughts/cached/tool 包含关系未核验，total 只取直报不派生".into(),
                ),
                "messages[].tokens: input/output/cached/thoughts/tool/total",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "tokens.cached reported（可选字段）",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Unavailable("格式内无缓存创建字段".into()),
                "无",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "每条带 tokens 的 gemini 消息按一次模型调用计（message.id 身份）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "消息自带 model 字段（request_field）；缺失 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial("文档级证据，待真实样本".into()),
                "消息 ISO8601 timestamp，source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("本地无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable(
                    "会话 JSON 无逐次延迟字段（telemetry api_response 有 duration_ms，未接入）"
                        .into(),
                ),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "gemini".to_string(),
            product: "Gemini CLI".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["<home>/.gemini"],
                "env_override": null,
                "manual_roots": true,
                "bounded": true,
                "pattern": "tmp/<project_hash>/chats/session-*.json",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "文件头 64 KiB 指纹：JSON object 且含 sessionId+messages 键（剥 UTF-8 BOM）",
                "version_field": "无版本字段；格式版本为文档级 session-doc-1",
                "fail_closed": true,
                "unknown_version": "未文档化消息 type 或载体外 tokens：整文件拒绝，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "gemini 消息 tokens（final，message.id 身份）",
                "cumulative_snapshot": "会话 JSON 无累计快照；checkpoint/账单侧写不接入",
                "aborted": "格式内无证据；未观测",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "文档化形状无子 Agent 概念；info/error/warning 消息真实存在但未文档化，fail closed 待扩",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 已消费字节数（整写 JSON 全量重读，事件 upsert 幂等）",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）", "首采样变化（前缀改写）"],
                "budget": "单文件 32 MiB 有界读取；超限受限受控重试",
                "mid_write": "半程写入 parse 失败不推进游标，下轮确定性重试",
            }),
            dedup: serde_json::json!({
                "primary": "gemini:{sessionId}:{message.id}（实例命名空间）",
                "fallback": "gemini:{sessionId}:idx-{数组下标}（缺 id，记诊断；依赖 append-only）",
                "cross_source": "telemetry（gemini_cli.token.usage/api_response）未接入，不与会话 JSON 相加",
            }),
            integrity: serde_json::json!({
                "success_only": "格式内无失败调用证据；只统计带 tokens 的 gemini 消息",
                "hidden_calls": "未观测",
                "sampling": "未观测到采样",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
                "prompt_content": "只读白名单字段（sessionId/messages[].{id,type,model,timestamp,tokens}），正文不提取",
            }),
            maintenance: serde_json::json!({
                "parser_version": GEMINI_PARSER_VERSION,
                "format_evidence": "官方文档 A10（会话路径与顶层形状、token 分类、telemetry 字段）；消息 tokens 形状按文档分类实现，待真实样本",
                "upgrade_policy": "未文档化消息 type/tokens 形状偏离 fail closed，取得真实样本后扩展",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "无变化探测短路；有变化全量重读（32 MiB 有界）+ upsert 幂等",
                "pause_cancel": "文件间可停；单文件读取有界",
            }),
            limitations: vec![
                "全部字段口径为文档级证据（A10），本机无真实样本（not_found）；首份真实 fixture 到达后逐字段核验".into(),
                "tokens 六键包含关系（cached/thoughts/tool 是否子集）未核验：total 只取直报，thoughts/tool 不并入任何字段".into(),
                "info/error/warning 消息类型真实存在但未文档化：fail closed，待真实样本后扩展接受集".into(),
                "整写 JSON 每次变化全量重读（32 MiB 有界），成本随文件大小线性；靠 upsert 幂等保证不双计".into(),
                "缺 message.id 记录用数组下标身份，历史非 append-only 时可能串号".into(),
                "符号链接/junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_stripped() {
        let with_bom = b"\xEF\xBB\xBF{\"sessionId\": 1}";
        assert_eq!(strip_bom(with_bom), b"{\"sessionId\": 1}");
        let without = b"{}";
        assert_eq!(strip_bom(without), b"{}");
    }
}
