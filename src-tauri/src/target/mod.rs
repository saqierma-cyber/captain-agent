//! ① Target Manager — see manager.rs.
//!
//! Slice 3 status: implemented for ExePath / ExePrefix / ProcessName.
//! BundleId is V2 (requires Info.plist resolution).

pub mod manager;
pub mod store_targets;

pub use manager::{TargetManager, TargetStatus};
