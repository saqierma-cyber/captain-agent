//! Target — a monitored software entity (an app, a CLI tool, etc.).
//!
//! Slice 3 supports three match kinds; BundleId is reserved for V2 when we
//! teach the helper to resolve macOS .app bundle identifiers via Info.plist.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetMatchKind {
    /// Exact match on the spawned process's executable path,
    /// e.g. `/Applications/Cursor.app/Contents/MacOS/Cursor`.
    ExePath,
    /// Prefix match — good for catching all helpers inside a .app bundle,
    /// e.g. value `/Applications/Cursor.app/` catches Cursor + every helper.
    ExePrefix,
    /// Match on basename of exe — value `zsh` catches `/bin/zsh`, etc.
    ProcessName,
    /// macOS bundle id (e.g. `com.todesktop.230313mzl4w4u92`). Resolution
    /// is deferred to V2 (requires walking the .app to read Info.plist).
    BundleId,
}

impl TargetMatchKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetMatchKind::ExePath => "exe_path",
            TargetMatchKind::ExePrefix => "exe_prefix",
            TargetMatchKind::ProcessName => "process_name",
            TargetMatchKind::BundleId => "bundle_id",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "exe_path" => TargetMatchKind::ExePath,
            "exe_prefix" => TargetMatchKind::ExePrefix,
            "process_name" => TargetMatchKind::ProcessName,
            "bundle_id" => TargetMatchKind::BundleId,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub id: Option<i64>,
    pub name: String,
    pub match_kind: TargetMatchKind,
    pub match_value: String,
    pub enabled: bool,
    pub created_at: i64,
}
