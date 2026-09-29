//! Kiro 适配器（AWS，闭源；独立目录合同）。双载体：
//! ① CLI `~/.kiro/sessions/cli/*.json` user_turn_metadatas（按 turn 真实计数）；
//! ② kiro-cli `~/.local/share/kiro-cli/data.sqlite3` conversations_v2
//! request_metadata（逐请求毫秒时间戳）。IDE 载体（session.json/messages.jsonl）
//! 纯估算不实施；Auto agent 零计数与估算路径一律不采。

pub mod detect;
pub mod versions;

pub use detect::{KIRO_FORMAT, KIRO_SQLITE_FORMAT};
pub use versions::{cli_turns_v1, sqlite_v1};
pub use versions::{
    KIRO_FORMAT_VERSION, KIRO_SQLITE_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS,
};

/// Kiro 适配器（无状态）。
pub struct KiroAdapter;

impl Default for KiroAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroAdapter {
    pub fn new() -> Self {
        KiroAdapter
    }
}

impl crate::adapters::framework::SourceAdapter for KiroAdapter {
    fn adapter_id(&self) -> &'static str {
        "kiro"
    }

    fn agent(&self) -> &'static str {
        "kiro"
    }

    fn discover(
        &self,
        ctx: &crate::adapters::framework::DiscoverContext,
    ) -> Vec<crate::adapters::framework::DiscoveredRoot> {
        use crate::adapters::framework::{DiscoveredRoot, RootBasis};
        let mut roots: Vec<(std::path::PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = &ctx.home_dir {
            // 载体①：CLI 会话头。
            roots.push((
                home.join(".kiro").join("sessions").join("cli"),
                RootBasis::DefaultHome,
            ));
            // 载体②：kiro-cli 库（unix；macOS 备选 Application Support）。
            let xdg = ctx
                .env
                .get("XDG_DATA_HOME")
                .filter(|v| !v.trim().is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            roots.push((xdg.join("kiro-cli"), RootBasis::DefaultHome));
            roots.push((
                home.join("Library")
                    .join("Application Support")
                    .join("kiro-cli"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
            std::collections::BTreeSet::new();
        for (root, basis) in roots {
            // CLI 目录枚举 *.json；kiro-cli 目录定位 data.sqlite3；手工根三者兼容。
            let mut files = crate::adapters::framework::enumerate_files_bounded(&root, 1, &|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            });
            let db = root.join("data.sqlite3");
            if db.is_file() {
                files.push(db);
            } else if root.is_file() {
                files = vec![root.clone()];
            }
            files.retain(|f| f.is_file());
            if !files.is_empty() && seen.insert(root.clone()) {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &crate::adapters::framework::DiscoveredRoot) -> String {
        format!(
            "kiro@{}",
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
        // 按文件形态分派（探测结论已在 format_status；此处按内容判定，
        // 与 detect 同一指纹规则）。
        let mut magic = [0u8; 16];
        let is_sqlite = std::fs::File::open(&target.path)
            .and_then(|mut f| {
                use std::io::Read;
                f.read_exact(&mut magic)
            })
            .map(|_| magic.starts_with(b"SQLite format 3\0"))
            .unwrap_or(false);
        if is_sqlite {
            versions::sqlite_v1::scan(target, stored, limits, now_ms)
        } else {
            versions::cli_turns_v1::scan(target, stored, limits, now_ms)
        }
    }

    fn capability(&self) -> crate::adapters::framework::CapabilityTable {
        use crate::adapters::framework::{Availability, CapabilityTable};
        let note =
            "第三方解析器证据（tokscale 1d9a939）；闭源 + 本机未安装，待真实样本核验".to_string();
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, detail: &str| serde_json::json!({ "availability": availability, "note": detail });
        fields.insert(
            "tokens".into(),
            field(
                Availability::Partial(note.clone()),
                "CLI turn 四桶（input/output/cache_read/cache_write 计数）+ SQLite request_metadata 五桶（多别名组）；只采显式计数",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(
                Availability::Partial(note.clone()),
                "cache_read_input_token_count / cache_read_input_tokens 别名组",
            ),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(note.clone()),
                "cache_write_input_token_count / cache_write_input_tokens 别名组",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Partial(note.clone()),
                "SQLite 载体逐请求（毫秒时间戳）；CLI 载体按 turn 聚合（total_request_count 无事件字段承载）",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(note.clone()),
                "CLI：会话级 rts_model_state.model_info.model_id；SQLite：无模型字段（Unknown，如实标注）",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Partial(note.clone()),
                "CLI end_timestamp（量级判别秒/毫秒）；SQLite request_start/stream_end 毫秒",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable(
                    "metering credit 是计价单位（0.04 USD/credit 为第三方换算不采信）".into(),
                ),
                "不映射",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Partial(note.clone()),
                "SQLite 载体 request_start..stream_end 区间",
            ),
        );
        CapabilityTable {
            adapter_id: "kiro".to_string(),
            product: "Kiro（AWS，CLI + kiro-cli）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: versions::VERIFIED_VERSION_IMPLS
                .iter()
                .map(|(v, _)| v.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": [
                    "~/.kiro/sessions/cli（CLI 会话头 *.json；同 stem .jsonl 是转录不读）",
                    "~/.local/share/kiro-cli/data.sqlite3（unix）",
                    "~/Library/Application Support/kiro-cli/data.sqlite3（macOS 备选）",
                ],
                "env_override": null,
                "manual_roots": "cli 会话目录、kiro-cli 目录或 data.sqlite3 文件",
                "bounded": true,
                "pattern": "双载体按内容指纹分派（SQLite magic / session_id+user_turn_metadatas）",
                "profile": "无",
            }),
            detection: serde_json::json!({
                "magic": "SQLite magic + conversations_v2 表；或 JSON 含 session_id + user_turn_metadatas",
                "version_field": "无；文档级锚点 kiro-cli-turns-1 / kiro-cli-sqlite-1",
                "registry": "adapters/kiro/versions 注册表（两个输入类型）",
                "fail_closed": true,
                "unknown_version": "格式偏离 fail closed",
            }),
            fields,
            lifecycle: serde_json::json!({
                "no_estimation": "Auto agent 零计数、字节/4 折算、context_window 差额、metering 换算全部不采纳（第三方模块头自证 ESTIMATED）",
                "ide_carrier_excluded": "IDE session.json/messages.jsonl 载体在第三方证据中为纯估算：不实施",
            }),
            incremental: serde_json::json!({
                "cursor": "CLI 整写 JSON 字节游标；SQLite offset 恒 0（≤50k 行整读，事件键 upsert 幂等）",
            }),
            dedup: serde_json::json!({
                "primary": "kiro:<session>:turn:<index> / kiro-sqlite:<conversation>:<index>",
                "cross_carrier": "CLI 与 kiro-cli 两载体重叠无证据：实例分列，真实样本后补对账（如实标注）",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "估算时代的调用（Auto agent）不可见——如实留空不补零",
            }),
            maintenance: serde_json::json!({
                "parser_version": versions::cli_turns_v1::KIRO_CLI_PARSER_VERSION,
                "format_evidence": "tokscale 固定提交 1d9a939 sessions/kiro.rs（闭源第三方逆向证据）",
                "evidence_level": "third-party-parser（无本机样本）",
                "upgrade_policy": "真实样本后核验两载体形状与交叉重叠",
            }),
            scheduling: serde_json::json!({ "entry": "统一 run_adapter_scan" }),
            limitations: vec![
                "闭源 + 本机未安装：文档级实现".into(),
                "估算路径一律不采（Auto 零计数/字节折算/窗口差额）".into(),
                "CLI 与 kiro-cli 载体交叉重叠未证：暂分列，防/漏双计待真实样本对账".into(),
                "IDE 载体（.chat/execution/promptLogs）不实施（估算 + 快照对账证据不足）".into(),
            ],
        }
    }
}
