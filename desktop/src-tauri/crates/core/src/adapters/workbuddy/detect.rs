//! WorkBuddy session-format detection.
pub const FORMAT: &str = "tencent-buddy-session-doc1";

pub fn select(found: Option<&str>) -> super::versions::Selection {
    super::versions::select(found)
}
