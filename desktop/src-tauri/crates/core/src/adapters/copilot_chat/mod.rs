//! VS Code 内置 GitHub Copilot Chat 会话用量适配器（独立目录约定）。载体：
//! VS Code 原生聊天会话日志 `workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl`
//! （chatSessionOperationLog storageSchema **version 3**；本机 VS Code 1.140.0
//! 真实数据核验 2026-10-01）。
//!
//! 载体识别依据（microsoft/vscode 源码 + 本机实读，2026-10-01）：
//! - 落盘位置：`chatSessionStore.ts` → `workspaceStorageHome/<workspaceId>/
//!   chatSessions`（无工作区窗口为 `no-workspace/chatSessions`）；
//! - 行格式：`objectMutationLog.ts` Entry——kind 0 首行完整初始对象、
//!   kind 1 Set(k 路径, v)、kind 2 Push(k, v[], i=截断后长度)、kind 3 Delete；
//!   超过 1024 条后整体重写为新初始行（replace）；
//! - 逐请求字段（`chatModel.ts` toJSON + `chatSessionOperationLog.ts` schema）：
//!   `promptTokens` = **末次模型调用的输入 token**（IChatUsage.promptTokens
//!   描述最近一次调用）、`completionTokens` = **整 turn 跨调用累计输出**
//!   （chatModel._setUsage 逐调用累加，modelTotals 存在时取权威值）、
//!   `copilotCredits` = turn 级 Copilot credit（nano AIU 折算，非 token）、
//!   `elapsedMs`、`outputBuffer`、`modelTotals`（agent host 会话才提供的
//!   整 turn 逐模型 input/cached/output 总量）、`sessionCopilotCredits`；
//! - 数值来源：Copilot 扩展 `agentIntent.ts` 直传 API 响应
//!   `usage.prompt_tokens` / `completion_tokens` /
//!   `prompt_tokens_details.cached_tokens`。
//!
//! 统计范围（data-contract「请求、消息与累计值」）：
//! - 一个 user turn = usage_observation；toolCallRounds 带 ID 的调用
//!   才计为 model_call。turn 内多次模型调用无逐次 token，
//!   `toolCallRounds` 仅含逐轮模型/思考 token 与时间戳；
//! - 默认路径 input = 末次调用输入（**已知下界**，非整 turn 输入和）；
//!   output = 整 turn 累计。modelTotals 存在时为权威整轮总量，优先采用；
//! - credit（premium 额度概念）不入 token 统计（额度类经 quota_history 独立展示）。

pub mod detect;
pub mod versions;

pub use detect::COPILOT_CHAT_FORMAT;

/// VS Code Copilot Chat 适配器（无状态）。
pub struct CopilotChatAdapter;

impl Default for CopilotChatAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CopilotChatAdapter {
    pub fn new() -> CopilotChatAdapter {
        CopilotChatAdapter
    }
}

/// 候选 VS Code 变体（stable/Insiders）的 workspaceStorage 根。
fn default_workspace_storages(
    env: &std::collections::BTreeMap<String, String>,
    home: Option<&std::path::Path>,
) -> Vec<std::path::PathBuf> {
    let variants = ["Code", "Code - Insiders"];
    let mut out = Vec::new();
    if cfg!(windows) {
        if let Some(appdata) = env
            .get("APPDATA")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let base = std::path::PathBuf::from(appdata);
            for v in variants {
                out.push(base.join(v).join("User").join("workspaceStorage"));
            }
        }
    } else {
        let Some(home) = home else {
            return out;
        };
        if cfg!(target_os = "macos") {
            let base = home.join("Library").join("Application Support");
            for v in variants {
                out.push(base.join(v).join("User").join("workspaceStorage"));
            }
        } else {
            // Linux / 其他：XDG_CONFIG_HOME 优先，否则 ~/.config。
            let base = env
                .get("XDG_CONFIG_HOME")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            for v in variants {
                out.push(base.join(v).join("User").join("workspaceStorage"));
            }
        }
    }
    out
}

/// 把一个 workspaceStorage 根枚举为若干 chatSessions 根
/// （每个 `<hash>/chatSessions` 一个根；`no-workspace/chatSessions` 同理）。
fn chat_session_roots_under(storage_root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let files = crate::adapters::framework::enumerate_files_bounded(
        storage_root,
        2,
        &|p: &std::path::Path| {
            p.extension().and_then(|e| e.to_str()) == Some("jsonl")
                && p.parent()
                    .map(|d| d.file_name().and_then(|n| n.to_str()) == Some("chatSessions"))
                    .unwrap_or(false)
        },
    );
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for file in files {
        let Some(parent) = file.parent().map(|p| p.to_path_buf()) else {
            continue;
        };
        let key = crate::adapters::framework::normalize_path(&parent);
        if cfg!(windows) {
            // Windows 大小写别名归一（与 run_adapter_scan 的根去重同规则）。
            if !seen.insert(key.to_lowercase()) {
                continue;
            }
        } else if !seen.insert(key) {
            continue;
        }
        dirs.push(parent);
    }
    dirs
}

impl crate::adapters::framework::SourceAdapter for CopilotChatAdapter {
    fn should_scan_unchanged(&self, stored: &crate::adapters::framework::StoredScanState) -> bool {
        versions::session_log_v3::should_scan_unchanged(stored)
    }

    fn adapter_id(&self) -> &'static str {
        "copilot-chat"
    }

    /// 统计 Agent 名：与 otel 适配器对 VS Code Copilot Chat span 的
    /// service.name 归属（"vscode-copilot-chat"）一致，两载体同维度不分裂。
    fn agent(&self) -> &'static str {
        "vscode-copilot-chat"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        for storage in default_workspace_storages(&ctx.env, ctx.home_dir.as_deref()) {
            if storage.is_dir() {
                for dir in chat_session_roots_under(&storage) {
                    roots.push((dir, RootBasis::DefaultHome));
                }
            }
            if let Some(user) = storage.parent() {
                let empty = user.join("globalStorage").join("emptyWindowChatSessions");
                if empty.is_dir() {
                    roots.push((empty, RootBasis::DefaultHome));
                }
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
            // 手工根可为 chatSessions 目录、workspaceStorage 目录或任意父目录：
            // 直接含 *.jsonl 视为 chatSessions 根；否则有界枚举两级找 chatSessions。
            let direct: Vec<std::path::PathBuf> = match std::fs::read_dir(manual) {
                Ok(entries) => entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("jsonl")
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
            if !direct.is_empty() {
                roots.push((manual.clone(), RootBasis::Manual));
                continue;
            }
            for dir in chat_session_roots_under(manual) {
                roots.push((dir, RootBasis::Manual));
            }
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (dir, basis) in roots {
            let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut files: Vec<std::path::PathBuf> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("jsonl")
                        })
                        .collect();
                    files.sort();
                    files
                }
                Err(_) => continue,
            };
            // 单文件手工根仅授权该文件；显式目录或自动发现才包含同目录其他文件。
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
                let instance_root = source_root(&dir);
                if let Some(existing) = out
                    .iter_mut()
                    .find(|r: &&mut DiscoveredRoot| r.root == instance_root)
                {
                    existing.files.extend(files);
                    existing.files.sort();
                    existing.files.dedup();
                    continue;
                }
                out.push(DiscoveredRoot {
                    root: instance_root,
                    basis,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "copilot-chat@{}",
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
        versions::session_log_v3::scan(target, stored, limits, now_ms)
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial("promptTokens=末次调用输入（turn 输入下界）、completionTokens=整 turn 累计输出；modelTotals 存在时为权威整轮逐模型总量".into()),
                "input_total/output_total；默认路径无缓存细分",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial("仅 modelTotals.cachedTokens（agent host 会话）".into()),
                "默认路径未知不补零",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(Availability::Unavailable("载体无此字段".into()), "未知"),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(
                    "toolCallRounds 仅覆盖已落盘主循环；辅助/子 Agent 调用覆盖未知".into(),
                ),
                "逐 round ID 计 model_call；user turn 与逐模型汇总只计 usage_observation",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "result.metadata.resolvedModel / modelId（如 claude-opus-4-8）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "modelState.completedAt（完成毫秒），缺失回退请求 timestamp",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable(
                    "copilotCredits 是 credit/nano AIU 计量非 USD：不映射".into(),
                ),
                "不映射；额度经 quota_history 独立展示",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Available,
                "elapsedMs + result.timings.firstProgress",
            ),
        );
        CapabilityTable {
            adapter_id: "copilot-chat".to_string(),
            product: "GitHub Copilot Chat (VS Code 内置)".to_string(),
            surfaces: vec!["vscode".into()],
            supported_versions: vec!["3".to_string()],
            discovery: serde_json::json!({
                "default_roots": [
                    "%APPDATA%/Code/User/workspaceStorage/<hash>/chatSessions (win)",
                    "~/Library/Application Support/Code/User/workspaceStorage/<hash>/chatSessions (mac)",
                    "~/.config/Code/User/workspaceStorage/<hash>/chatSessions (linux)",
                    "…/no-workspace/chatSessions（无工作区窗口）"
                ],
                "env_override": "无（workspaceStorage 无官方环境变量；自定义 --user-data-dir 经手工根）",
                "manual_roots": "chatSessions 目录 / workspaceStorage 目录 / 单个 .jsonl 会话文件",
                "bounded": true,
                "pattern": "两级有界枚举 <hash>/chatSessions/*.jsonl + globalStorage/emptyWindowChatSessions；stable 与 Insiders 变体",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSON {kind:0, v:{version:3, sessionId, requests[]}}",
                "version_field": "v.version（3=KnownVersion；其他/缺失 LatestFallback）",
                "registry": "adapters/copilot_chat/versions 注册表（唯一条目）",
                "fail_closed": true,
                "unknown_version": "未知 version 走 latest 兼容尝试（列集探测层校验）",
            }),
            fields,
            lifecycle: serde_json::json!({
                "per_turn": "kv 增量日志：流式计数器多次覆盖，取重放终值；modelState 1/2/3（Complete/Cancelled/Failed）封口为 Final，0/4（Pending/NeedsInput）为 Partial",
                "storage_migration": "objectMutationLog 超 1024 条整体重写为新初始行：framework 代数裁决触发重扫，事件按 (sessionId, requestId) 键幂等去重",
            }),
            incremental: serde_json::json!({
                "cursor": "每次全量重放（文件经压缩有界；真实样本 5.25MB/最大行 837KB）；游标记录已消费字节",
                "rationale": "kv 更新是周期采样快照，不能按行增量拼接状态；全量重放 + ingest 同键 upsert（同内容 unchanged）幂等",
            }),
            dedup: serde_json::json!({
                "primary": "安装命名空间 + sessionId/requestId；modelTotals 按模型键，调用按 round ID；完整快照替换旧用量贡献",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "只计已落盘 toolCallRounds；inline、辅助、子 Agent 不承诺完整；逐轮 token 未知，轮次汇总单独贡献",
                "known_gaps": "completionTokens 累计对后端重报（isSameUsage 去重失效场景）可能高估——上游已注释承认；promptTokens 下界语义见 tokens 字段",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::session_log_v3::COPILOT_CHAT_PARSER_VERSION,
                "format_evidence": "microsoft/vscode 源码（chatModel.ts toJSON / chatSessionOperationLog.ts storageSchema v3 / objectMutationLog.ts Entry / chatSessionStore.ts 落盘位置 / agentIntent.ts 数值来源）+ 本机 VS Code 1.140.0 真实数据核验（10 请求 5.25MB）",
                "evidence_level": "real-data（本机 2026-10-01 全字段核对）",
                "upgrade_policy": "version 字段变化走 latest 兼容尝试，真实样本后锚定新版本",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "默认路径 input_total 是末次调用输入（turn 输入下界）：VSCode 载体不落盘逐调用输入和；modelTotals（agent host 会话）存在时按权威整轮总量替换".into(),
                "call_count 按已观测 toolCallRounds 的 ID 计数；token 由 usage_observation 贡献；输入下界不与整轮输出派生总 token".into(),
                "thinking tokens（toolCallRounds[].thinking.tokens）覆盖不全（最终轮等）：不入 output_reasoning，不并入派生总量".into(),
                "copilotCredits/sessionCopilotCredits 是 credit 计量（nano AIU 折算）：不入 token 统计；账户级额度经 copilot-user-cache.json → quota_history 独立展示".into(),
                "已核验的 Copilot file OTel 按主机/用户/会话/本地日替代原生贡献，保留原始记录；开启当天覆盖受限，其他未核验导出不自动叠加".into(),
                "行流式计数器由周期 saveState 落盘：中途崩溃/强杀可能留下非终值，末次观测后不再变化即为止损终值".into(),
            ],
        }
    }
}

/// 同安装的工作区与空窗口迁移共用命名空间，迟到副本不增加用量。
fn source_root(dir: &std::path::Path) -> std::path::PathBuf {
    if dir.file_name().and_then(|n| n.to_str()) == Some("chatSessions") {
        if let Some(storage) = dir
            .parent()
            .and_then(|p| p.parent())
            .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("workspaceStorage"))
        {
            return storage.parent().unwrap_or(storage).to_path_buf();
        }
    }
    if dir.file_name().and_then(|n| n.to_str()) == Some("emptyWindowChatSessions") {
        if let Some(user) = dir.parent().and_then(|p| p.parent()) {
            return user.to_path_buf();
        }
    }
    dir.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::framework::{DiscoverContext, RootBasis, SourceAdapter};

    /// 默认发现路径（平台分支各自构造 fixture）：workspaceStorage/<hash>/
    /// chatSessions/*.jsonl 与 no-workspace/chatSessions/*.jsonl 均可发现，
    /// chatSessions 外的 jsonl 不收。
    #[test]
    fn default_discovery_finds_chat_sessions_roots() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-copilot-chat-discover-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let storage_root = dir.join("Code").join("User").join("workspaceStorage");
        let ws_a = storage_root.join("hasha").join("chatSessions");
        let no_ws = storage_root.join("no-workspace").join("chatSessions");
        let stray = storage_root.join("hasha");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&no_ws).unwrap();
        std::fs::write(ws_a.join("sess-1.jsonl"), "{}\n").unwrap();
        std::fs::write(no_ws.join("sess-2.jsonl"), "{}\n").unwrap();
        std::fs::write(stray.join("not-a-session.jsonl"), "{}\n").unwrap();

        let mut env = std::collections::BTreeMap::new();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        if cfg!(windows) {
            env.insert(
                "APPDATA".to_string(),
                dir.join("appdata").to_string_lossy().to_string(),
            );
            // fixture 挂到伪 APPDATA 下（Code/User/workspaceStorage）。
            let src = dir
                .join("appdata")
                .join("Code")
                .join("User")
                .join("workspaceStorage");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::rename(&storage_root, &src).unwrap();
        } else if cfg!(target_os = "macos") {
            env.insert("HOME".to_string(), home.to_string_lossy().to_string());
            let dst = home
                .join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("workspaceStorage");
            std::fs::create_dir_all(&dst).unwrap();
            std::fs::rename(&storage_root, &dst).unwrap();
        } else {
            env.insert(
                "XDG_CONFIG_HOME".to_string(),
                dir.join("xdg").to_string_lossy().to_string(),
            );
            let dst = dir
                .join("xdg")
                .join("Code")
                .join("User")
                .join("workspaceStorage");
            std::fs::create_dir_all(&dst).unwrap();
            std::fs::rename(&storage_root, &dst).unwrap();
        }

        let adapter = CopilotChatAdapter::new();
        let roots = adapter.discover(&DiscoverContext {
            home_dir: Some(home),
            env,
            manual_roots: Vec::new(),
        });
        let files: Vec<String> = roots
            .iter()
            .flat_map(|r| r.files.iter())
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(
            files.contains(&"sess-1.jsonl".to_string()),
            "files={files:?}"
        );
        assert!(
            files.contains(&"sess-2.jsonl".to_string()),
            "files={files:?}"
        );
        assert!(
            !files.contains(&"not-a-session.jsonl".to_string()),
            "stray jsonl outside chatSessions must be ignored, files={files:?}"
        );
        assert!(roots.iter().all(|r| r.basis == RootBasis::DefaultHome));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 手工根三种形态：chatSessions 目录、workspaceStorage 目录、单个 jsonl。
    #[test]
    fn manual_root_accepts_dir_storage_root_or_file() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-copilot-chat-manual-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let chat = dir.join("wshash").join("chatSessions");
        std::fs::create_dir_all(&chat).unwrap();
        std::fs::write(chat.join("sess.jsonl"), "{}\n").unwrap();
        let adapter = CopilotChatAdapter::new();
        for manual in [
            chat.clone(),
            dir.join("wshash")
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap(),
            chat.join("sess.jsonl"),
        ] {
            let roots = adapter.discover(&DiscoverContext {
                home_dir: None,
                env: Default::default(),
                manual_roots: vec![manual],
            });
            assert_eq!(
                roots.len(),
                1,
                "manual root {:?} should yield exactly one root",
                roots
            );
            assert_eq!(roots[0].files.len(), 1);
            assert!(matches!(roots[0].basis, RootBasis::Manual));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
