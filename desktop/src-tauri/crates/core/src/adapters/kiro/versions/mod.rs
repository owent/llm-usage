//! Kiro 版本注册表（V30 目录合同）：两个输入类型各自登记。
//! 证据（第三方解析器 tokscale 1d9a939；闭源产品 AWS，本机未安装）：
//! - CLI `~/.kiro/sessions/cli/*.json` 会话头 user_turn_metadatas（按 turn 真实
//!   计数；Auto agent 常记 0 ⇒ 只采显式非零计数，估算路径不采纳）。
//! - kiro-cli `~/.local/share/kiro-cli/data.sqlite3` conversations_v2 的
//!   request_metadata（毫秒时间戳 + 五桶真实计数）。
//!
//! IDE session.json/messages.jsonl 载体在第三方证据中为纯估算：不实施。

pub mod cli_turns_v1;
pub mod sqlite_v1;

/// 当前格式实现标识（CLI turns；SQLite 见第二登记条目）。
pub const LATEST_IMPL_ID: &str = "cli_turns_v1";

/// 文档级格式版本。
pub const KIRO_FORMAT_VERSION: &str = "kiro-cli-turns-1";
/// kiro-cli SQLite 载体锚点。
pub const KIRO_SQLITE_FORMAT_VERSION: &str = "kiro-cli-sqlite-1";

/// 已验证支持的格式版本 → 格式实现（两个输入类型）。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("kiro-cli-turns-1", "cli_turns_v1"),
    ("kiro-cli-sqlite-1", "sqlite_v1"),
];

/// 版本分派结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

/// 按格式版本选择实现；探测与扫描共用本函数（V30）。
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
        None => Selection {
            impl_id: LATEST_IMPL_ID,
            basis: crate::domain::VersionBasis::LatestFallback,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    #[test]
    fn both_carriers_dispatch_known() {
        assert_eq!(
            select(Some("kiro-cli-turns-1")),
            Selection {
                impl_id: "cli_turns_v1",
                basis: VersionBasis::KnownVersion
            }
        );
        assert_eq!(
            select(Some("kiro-cli-sqlite-1")),
            Selection {
                impl_id: "sqlite_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_falls_back() {
        assert_eq!(select(Some("other")).basis, VersionBasis::LatestFallback);
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
    }
}
