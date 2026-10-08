//! Detect CodeBuddy sessions using CLI JSONL or extension index.json format markers.
//! Path-based dispatch is defined in [`super::CodeBuddyAdapter::detect`].
pub const FORMAT: &str = "tencent-buddy-session-doc1";
pub const EXT_FORMAT: &str = super::extension_store::EXT_FORMAT;

pub fn select(found: Option<&str>) -> super::versions::Selection {
    super::versions::select(found)
}
