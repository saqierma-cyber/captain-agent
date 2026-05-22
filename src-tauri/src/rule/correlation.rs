//! Correlation rules — time-window pattern matcher over findings.
//!
//! When a single-event rule fires, the rule engine notifies the correlation
//! evaluator. The evaluator keeps a per-correlation-rule rolling map of
//! `required_rule_id → last_fire_time`. After updating, if EVERY rule in
//! `require_all_rules` has fired within `window_seconds`, the correlation
//! rule fires too (producing a new finding with its own severity/message).

use captain_common::{Finding, FindingStatus, Rule, RuleType};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct CorrelationEngine {
    /// `correlation_rule_id → (required_rule_id → last_fire_time)`.
    state: HashMap<String, HashMap<String, Instant>>,
}

impl CorrelationEngine {
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }

    /// Called whenever a single-event finding is emitted. Returns any
    /// correlation findings whose conditions were just satisfied.
    pub fn on_finding(
        &mut self,
        triggering: &Finding,
        correlation_rules: &[Rule],
        now: Instant,
    ) -> Vec<Finding> {
        let mut new_findings = Vec::new();
        for rule in correlation_rules {
            if !rule.enabled || rule.rule_type != RuleType::Correlation {
                continue;
            }
            let required = match &rule.require_all_rules {
                Some(r) if !r.is_empty() => r,
                _ => continue,
            };
            if !required.contains(&triggering.rule_id) {
                continue;
            }
            let window = Duration::from_secs(rule.window_seconds.unwrap_or(30));

            let per_rule = self.state.entry(rule.id.clone()).or_default();
            per_rule.insert(triggering.rule_id.clone(), now);

            // Garbage-collect stale entries OUTSIDE this rule's window.
            per_rule.retain(|_, t| now.duration_since(*t) <= window);

            // Check: all required rules have fresh entries.
            let all_fresh = required.iter().all(|req| {
                per_rule
                    .get(req)
                    .map(|t| now.duration_since(*t) <= window)
                    .unwrap_or(false)
            });
            if all_fresh {
                // Fire. Clear THIS correlation's state so we don't immediately
                // re-fire on the next single-event finding within the window.
                per_rule.clear();
                new_findings.push(build_correlation_finding(rule, triggering, required));
            }
        }
        new_findings
    }
}

fn build_correlation_finding(rule: &Rule, triggering: &Finding, required: &[String]) -> Finding {
    let message = rule.message.clone().unwrap_or_else(|| {
        format!(
            "{} — fired by combination of [{}]",
            rule.id,
            required.join(", ")
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
        pid: triggering.pid,
        event_kind: "correlation".to_string(),
        event_summary: format!(
            "triggered by {} (req: {})",
            triggering.rule_id,
            required.join(" + ")
        ),
    }
}
