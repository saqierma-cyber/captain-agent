//! Metric rules — sliding-window event-rate threshold detection.
//!
//! Implements a per-rule rolling window count. Useful for catching
//! "agent went crazy" patterns: an LLM-driven shell spawning hundreds of
//! processes per minute, a script making thousands of DNS queries, etc.
//!
//! Supported metric names (matched against `Rule::metric`):
//!   process_spawn_per_window  — count of ProcessSpawn events
//!   file_write_per_window     — count of FileWrite events
//!   file_read_per_window      — count of FileRead events
//!   net_connect_per_window    — count of NetConnect events
//!   dns_query_per_window      — count of DnsQuery events
//!
//! After firing, the window is cleared so the rule won't immediately
//! re-fire on the next event within the same burst — it requires a
//! fresh burst that crosses the threshold again.

use captain_common::{Event, EventDetail, Finding, FindingStatus, Rule, RuleType};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub struct MetricEngine {
    /// per-rule rolling timestamps of matching events
    windows: HashMap<String, VecDeque<Instant>>,
}

impl MetricEngine {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    /// Called for every event after target-filter passes. Returns any
    /// metric findings that just crossed their threshold.
    pub fn on_event(
        &mut self,
        ev: &Event,
        metric_rules: &[Rule],
        now: Instant,
    ) -> Vec<Finding> {
        let mut out = Vec::new();
        for rule in metric_rules {
            if !rule.enabled || rule.rule_type != RuleType::Metric {
                continue;
            }
            let metric = match &rule.metric {
                Some(m) => m.as_str(),
                None => continue,
            };
            if !event_matches_metric(ev, metric) {
                continue;
            }
            let window = Duration::from_secs(rule.window_seconds.unwrap_or(60));
            let threshold = rule.threshold.unwrap_or(u64::MAX) as usize;
            if threshold == 0 {
                continue;
            }

            let entry = self.windows.entry(rule.id.clone()).or_default();
            entry.push_back(now);
            // GC entries older than the window.
            while let Some(&t) = entry.front() {
                if now.duration_since(t) > window {
                    entry.pop_front();
                } else {
                    break;
                }
            }
            if entry.len() >= threshold {
                let count = entry.len();
                entry.clear();
                out.push(build_metric_finding(rule, ev, count));
            }
        }
        out
    }
}

fn event_matches_metric(ev: &Event, metric: &str) -> bool {
    match (metric, &ev.detail) {
        ("process_spawn_per_window", EventDetail::ProcessSpawn { .. }) => true,
        ("file_write_per_window", EventDetail::FileWrite { .. }) => true,
        ("file_read_per_window", EventDetail::FileRead { .. }) => true,
        ("net_connect_per_window", EventDetail::NetConnect { .. }) => true,
        ("dns_query_per_window", EventDetail::DnsQuery { .. }) => true,
        _ => false,
    }
}

fn build_metric_finding(rule: &Rule, ev: &Event, observed: usize) -> Finding {
    let message = rule
        .message
        .clone()
        .unwrap_or_else(|| {
            format!(
                "{} threshold hit ({} ≥ {} in {}s)",
                rule.metric.as_deref().unwrap_or("?"),
                observed,
                rule.threshold.unwrap_or(0),
                rule.window_seconds.unwrap_or(60)
            )
        });
    Finding {
        id: None,
        event_id: None,
        rule_id: rule.id.clone(),
        severity: rule.severity,
        message,
        status: FindingStatus::Open,
        notes: None,
        created_at: chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000),
        pid: ev.pid,
        event_kind: "metric".to_string(),
        event_summary: format!(
            "observed {} events in {}s",
            observed,
            rule.window_seconds.unwrap_or(60)
        ),
    }
}
