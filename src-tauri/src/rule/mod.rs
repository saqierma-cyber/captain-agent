//! ④ Rule engine — load rules, subscribe to bus, emit findings.
//!
//! Slice 4 adds correlation rules: when a single-event rule fires, the
//! correlation engine is notified; if it sees the full set of required
//! rules fire within `window_seconds`, it emits an additional finding
//! at the correlation rule's (typically higher) severity.

pub mod builtin;
pub mod correlation;
pub mod loader;
pub mod metric;
pub mod single;
pub mod store_rules;

use crate::bus::Bus;
use crate::notify::maybe_notify;
use crate::store::findings::insert_finding;
use crate::target::TargetManager;
use captain_common::{Finding, Rule, RuleType};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;

/// Dedupe window for single-event findings.
const DEDUP_WINDOW: Duration = Duration::from_secs(5);
const DEDUP_CACHE_CAP: usize = 2048;

/// Bucket rules into the three engines (single / correlation / metric).
fn categorize(rules: &[Rule]) -> (Vec<Rule>, Vec<Rule>, Vec<Rule>) {
    let enabled: Vec<&Rule> = rules.iter().filter(|r| r.enabled).collect();
    let single = enabled
        .iter()
        .filter(|r| !matches!(r.rule_type, RuleType::Correlation | RuleType::Metric))
        .copied()
        .cloned()
        .collect();
    let correlation = enabled
        .iter()
        .filter(|r| r.rule_type == RuleType::Correlation)
        .copied()
        .cloned()
        .collect();
    let metric = enabled
        .iter()
        .filter(|r| r.rule_type == RuleType::Metric)
        .copied()
        .cloned()
        .collect();
    (single, correlation, metric)
}

pub async fn run(
    pool: SqlitePool,
    bus: Bus,
    app_handle: AppHandle,
    rules_rx: tokio::sync::watch::Receiver<Vec<Rule>>,
    tm: Arc<TargetManager>,
) {
    let initial = rules_rx.borrow().clone();
    let (mut single_rules, mut correlation_rules, mut metric_rules) = categorize(&initial);
    tracing::info!(
        single = single_rules.len(),
        correlation = correlation_rules.len(),
        metric = metric_rules.len(),
        "rule engine started"
    );

    let mut rx = bus.subscribe();
    let mut rules_watcher = rules_rx;
    let mut recent: HashMap<(String, String, i64), Instant> = HashMap::new();
    let mut corr_engine = correlation::CorrelationEngine::new();
    let mut metric_engine = metric::MetricEngine::new();

    loop {
        tokio::select! {
            // Hot-reload: a Tauri command updated the rule set.
            r = rules_watcher.changed() => {
                if r.is_err() {
                    tracing::info!("rules watcher closed, rule engine exiting");
                    return;
                }
                let new_rules = rules_watcher.borrow().clone();
                let (s, c, m) = categorize(&new_rules);
                single_rules = s;
                correlation_rules = c;
                metric_rules = m;
                // Reset stateful engines so they re-evaluate from scratch
                // with the new rule set (no orphan history pointing to
                // rules that may have been deleted).
                recent.clear();
                corr_engine = correlation::CorrelationEngine::new();
                metric_engine = metric::MetricEngine::new();
                tracing::info!(
                    single = single_rules.len(),
                    correlation = correlation_rules.len(),
                    metric = metric_rules.len(),
                    "rule engine hot-reloaded rules"
                );
                continue;
            }
            evr = rx.recv() => match evr {
            Ok(ev) => {
                if !tm.keep_and_update(&ev) {
                    continue;
                }
                let now = Instant::now();
                if recent.len() > DEDUP_CACHE_CAP {
                    recent.retain(|_, t| now.duration_since(*t) < DEDUP_WINDOW);
                }
                // Metric rules: check every event regardless of single-rule
                // matches. They count rates, not patterns.
                for mut metric_finding in metric_engine.on_event(&ev, &metric_rules, now) {
                    if let Err(e) =
                        persist_and_emit(&pool, &app_handle, &mut metric_finding).await
                    {
                        tracing::error!(error = %e, "metric finding persist failed");
                    }
                }

                for rule in &single_rules {
                    let Some(mut finding) = single::evaluate(rule, &ev) else {
                        continue;
                    };
                    let key = (
                        finding.rule_id.clone(),
                        finding.event_summary.clone(),
                        finding.pid,
                    );
                    if let Some(prev) = recent.get(&key) {
                        if now.duration_since(*prev) < DEDUP_WINDOW {
                            continue;
                        }
                    }
                    recent.insert(key, now);

                    if let Err(e) = persist_and_emit(&pool, &app_handle, &mut finding).await {
                        tracing::error!(error = %e, "single finding persist failed");
                        continue;
                    }

                    // Feed into correlation engine.
                    let correlations = corr_engine.on_finding(&finding, &correlation_rules, now);
                    for mut corr in correlations {
                        if let Err(e) = persist_and_emit(&pool, &app_handle, &mut corr).await {
                            tracing::error!(error = %e, "correlation finding persist failed");
                        }
                    }
                }
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(dropped = n, "rule engine lagged");
            }
            Err(RecvError::Closed) => {
                tracing::info!("bus closed, rule engine exiting");
                return;
            }
            }
        }
    }
}

async fn persist_and_emit(
    pool: &SqlitePool,
    app_handle: &AppHandle,
    finding: &mut Finding,
) -> anyhow::Result<()> {
    let id = insert_finding(pool, finding).await?;
    finding.id = Some(id);
    if let Err(e) = app_handle.emit("finding", &*finding) {
        tracing::warn!(error = %e, "failed to emit finding");
    }
    maybe_notify(finding);
    Ok(())
}
