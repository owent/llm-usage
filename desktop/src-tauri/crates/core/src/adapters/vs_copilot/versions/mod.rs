//! VS Copilot 遥测版本注册表（V30 目录合同）。
//!
//! 锚点：本机 VS 18 Community 真实数据（2026-10-01）——OTLP JSON 信封
//! （行=resourceSpans 批次）、service.name=vs-copilot、chat span 携带
//! gen_ai.usage.*（intValue 字符串形）、纳秒 Unix 时间戳。

pub mod traces_v1;

/// 当前格式实现标识。
pub const LATEST_IMPL_ID: &str = "traces_v1";

/// 已验证的格式版本（本机真实数据核对 2026-10-01）。
pub const VS_COPILOT_FORMAT_VERSION: &str = "vs-copilot-otlp-traces-v1";

/// 已验证支持的格式版本 → 格式实现。
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[("1", "traces_v1")];

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
    fn v1_dispatches_known() {
        assert_eq!(
            select(Some("1")),
            Selection {
                impl_id: "traces_v1",
                basis: VersionBasis::KnownVersion
            }
        );
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(select(None).basis, VersionBasis::LatestFallback);
        assert_eq!(select(Some("2")).basis, VersionBasis::LatestFallback);
    }
}
