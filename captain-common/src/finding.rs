//! Findings — what the rule engine emits when an event matches a rule.
//! Persisted to the `findings` SQLite table; pushed live to the UI.

use crate::event::Severity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    /// New, awaiting user triage.
    Open,
    /// User confirmed this is genuinely suspicious.
    Confirmed,
    /// User looked at it and decided it was harmless this one time.
    Dismissed,
    /// User explicitly never wants this pattern flagged again
    /// (will be enforced by the rule engine via auto-generated user rule).
    Whitelisted,
}

impl FindingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingStatus::Open => "open",
            FindingStatus::Confirmed => "confirmed",
            FindingStatus::Dismissed => "dismissed",
            FindingStatus::Whitelisted => "whitelisted",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(FindingStatus::Open),
            "confirmed" => Some(FindingStatus::Confirmed),
            "dismissed" => Some(FindingStatus::Dismissed),
            "whitelisted" => Some(FindingStatus::Whitelisted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// DB id, None for findings still in flight.
    pub id: Option<i64>,
    /// Linked event id — None until both event + finding are persisted.
    /// Slice 2: we set None and rely on (pid, ts) to correlate in UI.
    pub event_id: Option<i64>,
    /// Which rule matched.
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub status: FindingStatus,
    /// Optional user-added notes.
    pub notes: Option<String>,
    /// Unix epoch nanoseconds.
    pub created_at: i64,
    // ─── Denormalized for fast UI display ──────────────────
    /// Originating event's PID (so UI can show "Cursor pid=12345").
    pub pid: i64,
    /// Event kind, denormalized so the UI doesn't need to JOIN.
    pub event_kind: String,
    /// Event summary (path / cmdline / remote_addr — whichever applies).
    pub event_summary: String,
}
