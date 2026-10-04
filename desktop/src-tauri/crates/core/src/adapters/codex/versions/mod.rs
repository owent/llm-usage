//! Codex 版本注册表：发布版本 → 格式实现的映射与未知版本回退选择
//! （architecture.md#adapter-layout / #unknown-version）。
//!
//! 已验证版本须经真实样本核验（fixture 或本机全量实读 + 期望值核算）；
//! 注册表扩展只增加条目，不删除历史实现。
//! 格式实现：
//! - `rollout_v1`：0.153+ 逐次载体 `token_usage_record`（0.155.0-alpha.16.3
//!   真实 fixture 核验读取、解析、入库与查询；0.153/0.154 逐版本 fixture 同形共用）；
//! - `rollout_legacy`：0.139–0.151 无逐次载体，以 `event_msg/token_count` 的
//!   `last_token_usage` 回声提供逐次用量（2026-09-26 本机全量 238 文件实读核验：
//!   21 版本零 token_usage_record；13,481 条 token_count 按 total 增量法分桶；
//!   222/238 对账 matched；核验方法与判据详见 rollout_legacy.rs 文件头）。
//!
//! 选择规则：
//! - 已收录版本 → `KnownVersion`，按映射分派；
//! - 未收录/缺失版本 → `LatestFallback`，先尝试最新内置解析器（rollout_v1），
//!   通过校验的数据带兼容标记入库，不因版本号未收录直接拒绝；
//! - Codex 无已证实不兼容的版本（无 `known_incompatible` 条目）；
//!   结构不兼容在扫描层按 V30 判定并保留旧结果。

pub mod rollout_legacy;
pub mod rollout_v1;

/// 当前格式实现标识（"最新内置解析器"由本常量明确指定，不联网获取）。
pub const LATEST_IMPL_ID: &str = "rollout_v1";

/// 已验证支持的发布版本 → 格式实现。
/// 每个版本都有固定格式样本并已核验；同形版本共用实现，分派仍逐版本登记。
/// 0.139–0.151 系列（rollout_legacy）：本机全量实读核验（每版本全文件
/// total 增量法分桶 + Σ逐次 vs 最终快照核算，222/238 matched、16 mismatch
/// 均属已解释类别），代表 fixture 0.139.0 / 0.142.5 / 0.146.0-alpha.3
/// 读取、解析、入库与查询均已验证；详情见 rollout_legacy.rs 文件头与 m2d 验证记录。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    // M2-A 真实 fixture（3 会话，49 调用读取、解析、入库与查询的期望值）。
    ("0.155.0-alpha.16.3", "rollout_v1"),
    // M2-D 逐版本脱敏 fixture（rollout-v*.sanitized.json + _expectations.md
    // jq 独立核算；token_usage_record 逐次载体与 0.155 同形，共用 rollout_v1）。
    ("0.154.0-alpha.6.2", "rollout_v1"),
    ("0.154.0-alpha.6.1", "rollout_v1"),
    ("0.153.0", "rollout_v1"),
    // 旧载体系列（token_count/last_token_usage）：本机 238 文件实读核验 +
    // rollout-legacy-*.sanitized.json fixture 验证读取、解析、入库与查询（2026-09-26）。
    ("0.151.0-alpha.7.1", "rollout_legacy"),
    ("0.149.0-alpha.4.1", "rollout_legacy"),
    ("0.148.0-alpha.9", "rollout_legacy"),
    ("0.147.0-alpha.6.5", "rollout_legacy"),
    ("0.146.0-alpha.9.2", "rollout_legacy"),
    ("0.146.0-alpha.3.1", "rollout_legacy"),
    ("0.146.0-alpha.3", "rollout_legacy"),
    ("0.145.0-alpha.27", "rollout_legacy"),
    ("0.145.0-alpha.18", "rollout_legacy"),
    ("0.144.5", "rollout_legacy"),
    ("0.144.2", "rollout_legacy"),
    ("0.144.0-alpha.4", "rollout_legacy"),
    ("0.142.5", "rollout_legacy"),
    ("0.142.4", "rollout_legacy"),
    ("0.142.3", "rollout_legacy"),
    ("0.142.2", "rollout_legacy"),
    ("0.142.0", "rollout_legacy"),
    ("0.142.0-alpha.6", "rollout_legacy"),
    ("0.142.0-alpha.1", "rollout_legacy"),
    ("0.140.0-alpha.2", "rollout_legacy"),
    ("0.139.0", "rollout_legacy"),
];

/// 兼容公开路径的旧常量名（tests/examples 引用）。
pub const SUPPORTED_CLI_VERSIONS: &[&str] = &["0.155.0-alpha.16.3"];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按来源原始版本选择格式实现；探测与扫描共用本函数保证同一策略（V30）。
pub fn select(found: Option<&str>) -> Selection {
    match found {
        Some(version) => {
            let known = VERIFIED_VERSION_IMPLS
                .iter()
                .find(|(v, _)| *v == version)
                .map(|(_, impl_id)| *impl_id);
            match known {
                Some(impl_id) => Selection {
                    impl_id,
                    basis: crate::domain::VersionBasis::KnownVersion,
                },
                None => Selection {
                    impl_id: LATEST_IMPL_ID,
                    basis: crate::domain::VersionBasis::LatestFallback,
                },
            }
        }
        // 版本字段缺失但 Agent 身份/输入类型已确认：默认回退最新实现。
        None => Selection {
            impl_id: LATEST_IMPL_ID,
            basis: crate::domain::VersionBasis::LatestFallback,
        },
    }
}

/// 为一次扫描解析选择依据：优先用已持久化解析上下文里的 cli_version
/// （增量轮次免重复读首行）；首轮/重扫/上下文缺失时有界读首行 session_meta。
/// 任一路径失败都回到 `select(None)` = LatestFallback → rollout_v1（与探测同策略）。
fn select_for_scan(
    target: &crate::adapters::framework::ScanTarget,
    stored: &crate::adapters::framework::StoredScanState,
) -> Selection {
    let from_context = if target.rescan {
        None
    } else {
        stored
            .parse_context
            .as_ref()
            .and_then(|v| v.get("cli_version"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    match from_context {
        Some(version) => select(Some(version.as_str())),
        None => select(first_line_cli_version(&target.path).as_deref()),
    }
}

/// 有界读首行 session_meta.payload.cli_version（读取失败/非 JSON 均返回 None）。
fn first_line_cli_version(path: &std::path::Path) -> Option<String> {
    let limits = crate::adapters::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: crate::adapters::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = crate::adapters::jsonl::read_jsonl(path, 0, 1, &limits).ok()?;
    let first = outcome.lines.first()?;
    let line = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text).ok()?;
    if line.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
        return None;
    }
    line.get("payload")
        .and_then(|p| p.get("cli_version"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// 统一扫描分派：按注册表选择实现（探测/扫描同一注册表，V30）。
/// 已知限制（与 latest_fallback 升级限制同源）：跨实现升级不自动重扫已消费游标；
/// 本系列旧文件此前均判 incompatible 且游标未推进，登记后从头解析无残留状态。
pub fn dispatch_scan(
    target: &crate::adapters::framework::ScanTarget,
    stored: &crate::adapters::framework::StoredScanState,
    limits: &crate::adapters::framework::ScanLimits,
    now_ms: i64,
) -> Result<crate::adapters::framework::ScanOutcome, crate::error::CoreError> {
    match select_for_scan(target, stored).impl_id {
        "rollout_legacy" => rollout_legacy::scan(target, stored, limits, now_ms),
        _ => rollout_v1::scan(target, stored, limits, now_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn verified_versions_dispatch_known() {
        assert_eq!(
            select(Some("0.155.0-alpha.16.3")),
            Selection {
                impl_id: "rollout_v1",
                basis: VersionBasis::KnownVersion
            }
        );
        // M2-D 逐版本 fixture 核验的同形版本。
        for v in ["0.153.0", "0.154.0-alpha.6.1", "0.154.0-alpha.6.2"] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "rollout_v1",
                    basis: VersionBasis::KnownVersion
                }
            );
        }
        // 0.139–0.151 系列已核验并登记为 rollout_legacy（fixture + 全量实读）。
        for v in [
            "0.139.0",
            "0.140.0-alpha.2",
            "0.142.0-alpha.1",
            "0.142.0-alpha.6",
            "0.142.0",
            "0.142.2",
            "0.142.3",
            "0.142.4",
            "0.142.5",
            "0.144.0-alpha.4",
            "0.144.2",
            "0.144.5",
            "0.145.0-alpha.18",
            "0.145.0-alpha.27",
            "0.146.0-alpha.3",
            "0.146.0-alpha.3.1",
            "0.146.0-alpha.9.2",
            "0.147.0-alpha.6.5",
            "0.148.0-alpha.9",
            "0.149.0-alpha.4.1",
            "0.151.0-alpha.7.1",
        ] {
            assert_eq!(
                select(Some(v)),
                Selection {
                    impl_id: "rollout_legacy",
                    basis: VersionBasis::KnownVersion
                },
                "version {v}"
            );
        }
        // 未核验版本（如 0.141.0、未来版本）仍按未知版本回退 rollout_v1。
        assert_eq!(select(Some("0.141.0")).basis, VersionBasis::LatestFallback);
        assert_eq!(
            select(Some("0.142.5-x")).basis,
            VersionBasis::LatestFallback
        );
    }

    #[test]
    fn unrecorded_version_falls_back_to_latest() {
        assert_eq!(
            select(Some("0.199.0-alpha.1")),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
        assert_eq!(
            select(None),
            Selection {
                impl_id: LATEST_IMPL_ID,
                basis: VersionBasis::LatestFallback
            }
        );
    }
}
