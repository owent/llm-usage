//! CodeBuddy 会话格式探测入口。CLI JSONL 与扩展存储 index.json 两个格式锚点，
//! 路径形状分派见 [`super::CodeBuddyAdapter::detect`]。
pub const FORMAT: &str = "tencent-buddy-session-doc1";
pub const EXT_FORMAT: &str = super::extension_store::EXT_FORMAT;

pub fn select(found: Option<&str>) -> super::versions::Selection {
    super::versions::select(found)
}
