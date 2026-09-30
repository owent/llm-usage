//! 闭源 JSONL 无格式版本字段；只承诺已核验的文档级格式锚点。
pub const LATEST_IMPL_ID: &str = "session_doc1";
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] =
    &[("tencent-buddy-session-doc1", LATEST_IMPL_ID)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub impl_id: &'static str,
    pub basis: crate::domain::VersionBasis,
}

pub fn select(found: Option<&str>) -> Selection {
    let known =
        found.and_then(|version| VERIFIED_VERSION_IMPLS.iter().find(|(v, _)| *v == version));
    Selection {
        impl_id: known
            .map(|(_, implementation)| *implementation)
            .unwrap_or(LATEST_IMPL_ID),
        basis: if known.is_some() {
            crate::domain::VersionBasis::KnownVersion
        } else {
            crate::domain::VersionBasis::LatestFallback
        },
    }
}
