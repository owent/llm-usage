//! Read WorkBuddy local projects sessions separately from CodeBuddy OTLP.
pub mod detect;
pub mod versions;
pub use crate::adapters::tencent_buddy_wire::BuddyAdapter as BuddyAdapterGeneric;
pub type WorkBuddyAdapter = BuddyAdapterGeneric<true>;
