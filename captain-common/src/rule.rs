//! Rule definitions shared by the rule engine + YAML loaders + UI.
//!
//! Slice 2 supports only single-event rules (file / process / network).
//! Correlation + metric rules come in Slice 4.

use crate::event::Severity;
use serde::{Deserialize, Serialize};

/// One YAML file contains a rule pack with multiple rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RulePack {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// Match against file_read / file_write / file_delete events.
    File,
    /// Match against process_spawn events (regex on cmdline).
    Process,
    /// Match against net_connect / dns_query events.
    Network,
    /// Time-window combination of other rules — Slice 4.
    Correlation,
    /// Rolling aggregate threshold — Slice 4.
    Metric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(rename = "type")]
    pub rule_type: RuleType,
    pub severity: Severity,
    #[serde(default)]
    pub message: Option<String>,

    // ─── File rule fields ─────────────────────────────────
    /// Glob pattern, e.g. `~/.ssh/*` or `/Users/*/.aws/credentials`.
    /// `~` and `~/` are expanded to `/Users/*/`.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Optional action filter: "read" / "write" / "delete". None = any.
    #[serde(default)]
    pub action: Option<String>,

    // ─── Process rule fields ──────────────────────────────
    /// Regex against the full cmdline string.
    #[serde(default)]
    pub cmd_pattern: Option<String>,

    // ─── Network rule fields ──────────────────────────────
    /// Substring match against domain (for DnsQuery) or remote_addr (NetConnect).
    /// Any match in this list triggers the rule.
    #[serde(default)]
    pub domains: Option<Vec<String>>,

    // ─── Correlation rule fields (Slice 4) ─────────────────
    /// List of rule_ids that must all have fired within `window_seconds`
    /// for this correlation rule to fire. Order doesn't matter.
    #[serde(default)]
    pub require_all_rules: Option<Vec<String>>,
    /// Sliding window for correlation / metric rules.
    #[serde(default)]
    pub window_seconds: Option<u64>,

    // ─── Metric rule fields (Slice 5) ──────────────────────
    /// Name of the metric this rule watches. Currently supported:
    ///   `process_spawn_per_window` / `file_write_per_window` /
    ///   `net_connect_per_window` / `dns_query_per_window` /
    ///   `file_read_per_window`.
    #[serde(default)]
    pub metric: Option<String>,
    /// If the metric's rolling count over `window_seconds` reaches this
    /// value, the rule fires.
    #[serde(default)]
    pub threshold: Option<u64>,

    /// Whether the rule is currently active. User rules can disable
    /// built-in rules by id.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
