//! DSH: rc.2 native v4 snapshots and the separate historical synthetic doc1 reader.
//! Actual installed persistence/LLM/token-meter evidence: m3-runtime-samples.md.
pub mod common;
pub mod detect;
pub mod versions;
pub use common::{map_dsh_usage, DshUsage};
pub use detect::DSH_FORMAT;
pub use versions::session_v4::PARSER_VERSION as DSH_PARSER_VERSION;
pub use versions::{session_log_doc1, DSH_FORMAT_VERSION, LATEST_IMPL_ID, VERIFIED_VERSION_IMPLS};

use crate::adapters::framework::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct DshAdapter;
impl Default for DshAdapter {
    fn default() -> Self {
        Self::new()
    }
}
impl DshAdapter {
    pub fn new() -> Self {
        Self
    }
}

fn root_for(path: &Path) -> PathBuf {
    path.ancestors()
        .skip(1)
        .find(|p| p.file_name().is_some_and(|n| n == "sessions"))
        .unwrap_or_else(|| path.parent().unwrap_or(path))
        .to_path_buf()
}
fn resolve_home(value: &str, home: Option<&Path>) -> Option<PathBuf> {
    if value == "~" {
        return home.map(Path::to_path_buf);
    }
    if let Some(suffix) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home.map(|h| h.join(suffix));
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Some(path)
    } else {
        std::env::current_dir().ok().map(|p| p.join(path))
    }
}
fn files(root: &Path, manual: bool) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let candidates = enumerate_files_bounded(root, 3, &|p| {
        versions::session_v4::generation(p).is_some()
            || manual && p.extension().is_some_and(|x| x == "jsonl")
    });
    // A released client selects the latest immutable generation in each session directory.
    // An unsupported future generation must not fall back to historical v4 counts.
    let mut latest: BTreeMap<PathBuf, u64> = BTreeMap::new();
    for p in &candidates {
        if let Some(v) = versions::session_v4::generation(p) {
            latest
                .entry(p.parent().unwrap().to_path_buf())
                .and_modify(|old| *old = (*old).max(v))
                .or_insert(v);
        }
    }
    candidates
        .into_iter()
        .filter(|p| {
            versions::session_v4::generation(p).map_or(true, |v| latest[p.parent().unwrap()] == v)
        })
        .collect()
}
impl SourceAdapter for DshAdapter {
    fn adapter_id(&self) -> &'static str {
        "dsh"
    }
    fn agent(&self) -> &'static str {
        "deepseek-harness"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        let mut candidates = vec![];
        if let Some(value) = ctx.env.get("DSH_HOME").filter(|s| !s.trim().is_empty()) {
            if let Some(home) = resolve_home(value, ctx.home_dir.as_deref()) {
                candidates.push((
                    home.join("sessions"),
                    RootBasis::EnvOverride("DSH_HOME".into()),
                    false,
                ));
            }
        } else if let Some(home) = &ctx.home_dir {
            candidates.push((home.join(".dsh/sessions"), RootBasis::DefaultHome, false));
        }
        for manual in &ctx.manual_roots {
            let path = if manual.join("sessions").is_dir() {
                manual.join("sessions")
            } else if manual.join(".dsh/sessions").is_dir() {
                manual.join(".dsh/sessions")
            } else {
                manual.clone()
            };
            candidates.push((path, RootBasis::Manual, true));
        }
        let mut roots: BTreeMap<String, DiscoveredRoot> = BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        for (candidate, basis, manual) in candidates {
            for path in files(&candidate, manual) {
                let canonical = normalize_path(&path);
                if !seen.insert(canonical) {
                    continue;
                }
                let root = if versions::session_v4::is_native_path(&path) {
                    root_for(&path)
                } else {
                    candidate.clone()
                };
                roots
                    .entry(normalize_path(&root))
                    .or_insert_with(|| DiscoveredRoot {
                        root,
                        basis: basis.clone(),
                        files: vec![],
                    })
                    .files
                    .push(path);
            }
        }
        roots.into_values().collect()
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!("dsh@{}", normalize_path(&root.root))
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, crate::error::CoreError> {
        detect::detect(path)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, crate::error::CoreError> {
        // Detection owns arbitrary manual filenames; schema framing, never bloodline, dispatches.
        match detect::detect(&target.path)? {
            DetectOutcome::Supported { format, .. } if format == versions::session_v4::FORMAT => {
                versions::session_v4::scan(target, stored, limits, now_ms)
            }
            _ if !versions::session_v4::is_native_path(&target.path) => {
                versions::session_log_doc1::scan(target, stored, limits, now_ms)
            }
            _ => versions::session_v4::scan(target, stored, limits, now_ms),
        }
    }
    fn capability(&self) -> CapabilityTable {
        let mut fields = serde_json::Map::new();
        for (name,note) in [
            ("tokens","rc.2 v4 positive input/output/cache; verified own pi-ai openai-completions response restores inclusive input; default zeros unknown"),
            ("cache_read","native positive cacheReadTokens; missing/default zero unknown"),
            ("cache_write","native positive cacheWriteTokens; positive path source-backed, real sample has none"),
            ("per_request_calls","one observed settlement attempt per turn/step/retry boundary; message/stream copies folded once"),
            ("model","only that assistant message.source provider/model; failed attempt without own model stays unknown"),
            ("time","durable event.time (observed_at), not exact upstream request start/end"),
        ] {fields.insert(name.into(),serde_json::json!({"availability":Availability::Partial("real-container rc.2 v4, limited route".into()),"note":note}));}
        for (name, note) in [
            (
                "cost",
                "no validated persisted amount; pi-ai default zero is not billing",
            ),
            ("latency", "no validated request latency/TTFT"),
        ] {
            fields.insert(name.into(),serde_json::json!({"availability":Availability::Unavailable(note.into()),"note":note}));
        }
        CapabilityTable {
            adapter_id:"dsh".into(),product:"DeepSeek Harness (DSH)".into(),surfaces:vec!["harness".into()],
            supported_versions:VERIFIED_VERSION_IMPLS.iter().map(|(v,_)|v.to_string()).collect(),
            discovery:serde_json::json!({"default_roots":["~/.dsh/sessions"],"env_override":"DSH_HOME (native tilde/CWD resolution)","manual_roots":"home/sessions/workspace/session/file; canonical physical dedup","bounded":true,"pattern":"<sessions>/<encoded-cwd>/<session>/session.v4.jsonl[.zstd]","generation":"highest canonical generation; unsupported future generation never falls back"}),
            detection:serde_json::json!({"magic":"exact v4 session header, dense seq/time/data rows; separate doc1 synthetic fingerprint","version_field":"physical format 4, not client product version","fail_closed":true,"unknown_version":"unsupported physical generations require evidence"}),
            fields,
            lifecycle:serde_json::json!({"settlement":"data.usage else last stream usage; only latest sample in the attempt","retry":"llm/retry-started creates a distinct identity","inheritance":"last inherited end-seed cut excluded; ordinary empty resume marker retains history","zeros":"default zero buckets unknown","source_total":"pi-ai total is self-calculated, not independent reported total"}),
            incremental:serde_json::json!({"cursor":"only complete valid bounded snapshot; full fold on changed bytes","caps":"64 MiB file/decoded/window; 4 MiB line; 50,000 lines; cooperative/time limits","half_write":"no events/checkpoint commit","revision":"persisted complete digest + monotonic revision floor","regression":"missing prior attempt or changed native session identity retains old checkpoint and emits diagnostic"}),
            dedup:serde_json::json!({"primary":"native session/turn/step/retry boundary in canonical source instance","copies":"message/stream/context-pressure not added","transaction":"events/revisions/context/checkpoint/summary same transaction"}),
            integrity:serde_json::json!({"success_only":false,"bad_usage":"observed attempt unknown with diagnostic; valid neighbors remain available","hidden_calls":"title/auxiliary result usage may be absent; never fill from API differences","prompt_content":"only event/identity/time/model/usage whitelist retained"}),
            maintenance:serde_json::json!({"parser_version":DSH_PARSER_VERSION,"evidence_level":"real-container @deepseek-ai/dsh 0.2.0-rc.2, native v4; installed locked modules and pi-ai 0.87.1","legacy_evidence":"doc1 README 46a7f68 JSONL 行形状仍为合成假设，独立保留，不认证 rc.2","upgrade_policy":"reassess unchanged old checkpoints; only native format, not all client versions, validated"}),
            scheduling:serde_json::json!({"entry":"unified bounded local scan","pause_cancel":"cooperative reads and row folds; stopped snapshot never advances cursor"}),
            limitations:vec![
                "仅 rc.2 v4 两轮 OpenAI-compatible 本地模型真实验收；其他协议、CLI/UI 面和版本未认证".into(),
                "标题 API 有一次调用但持久化无结果用量，主循环两次之外的费用/token 覆盖受限".into(),
                "种子/重试/失败 settlement 按分发源码实施和合成回归；本轮未生成这些路径的真实样本".into(),
                "只校验采集所需封套、生命周期与用量归属；不代替 DSH 全量工具/压缩/交付关系恢复器".into(),
                "有界完整快照；历史 attempt 删除/身份改写拒绝推进并保留旧结果，尚无源端删除纠正合同".into(),
                "旧 doc1 只具文档级依据；不把合成行或无用量空会话认证为真实版本".into(),
            ],
        }
    }
}
