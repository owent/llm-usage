//! 闭源 JSONL 无格式版本字段；只承诺已核验的格式锚点。
//! CLI JSONL 为文档级（第三方源码）；扩展存储 index.json 为 2026-09-30 本机核验。
pub const LATEST_IMPL_ID: &str = "session_doc1";
pub const EXT_LATEST_IMPL_ID: &str = "extension_requests_doc1";
pub const VERIFIED_VERSION_IMPLS: &[(&str, &str)] = &[
    ("tencent-buddy-session-doc1", LATEST_IMPL_ID),
    ("codebuddy-extension-requests-doc1", EXT_LATEST_IMPL_ID),
];

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
