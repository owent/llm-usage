//! VS Code Copilot Chat 会话日志版本注册表（V30 目录合同）。
//!
//! 锚点：chatSessionOperationLog.ts storageSchema 固定 `version: 3`
//! （microsoft/vscode 源码）+ 本机 VS Code 1.140.0 真实数据核对
//! （2026-10-01，10 请求全字段核验）。version=3 → KnownVersion；
//! 其他/缺失 → LatestFallback 兼容尝试。

pub mod session_log_v3;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "session_log_v3";

/// 已验证的格式版本（本机真实数据核对 2026-10-01）。
pub const COPILOT_CHAT_FORMAT_VERSION: &str = "vscode-chat-session-log-v3";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("3", "session_log_v3")];

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
    fn v3_dispatches_known() {
        assert_eq!(
            select(Some("3")),
            Selection {
                impl_id: "session_log_v3",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("4")).basis, VersionBasis::LatestFallback);
    }
}
