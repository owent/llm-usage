//! WorkBuddy 的独立本地 projects 会话载体；不沿用 CodeBuddy OTLP 来源。
pub mod detect;
pub mod versions;
pub use crate::adapters::tencent_buddy_wire::BuddyAdapter as BuddyAdapterGeneric;
pub type WorkBuddyAdapter = BuddyAdapterGeneric<true>;
