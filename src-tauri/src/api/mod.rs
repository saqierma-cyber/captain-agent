//! ⑥ UI backend API — Tauri commands (request/response) + event channel
//! (server-pushed live events).

pub mod dashboard;
pub mod reports;
pub mod settings;

use crate::bus::Bus;
use crate::rule;
use crate::store::findings;
use crate::target::{TargetManager, TargetStatus};
use captain_common::{Event, Finding, FindingStatus, Rule, Target, TargetMatchKind};
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::broadcast::error::RecvError;

/// Type alias for the Tauri-managed watch sender used to hot-reload
/// rules in the rule engine.
pub type RulesSender = Arc<tokio::sync::watch::Sender<Vec<Rule>>>;

/// Long-running task: subscribe to the bus and forward each event to the UI,
/// filtered through the Target Manager (which also updates monitored PID
/// set on ProcessSpawn).
pub async fn push_events_to_ui(bus: Bus, app_handle: AppHandle, tm: Arc<TargetManager>) {
    let mut rx = bus.subscribe();
    loop {
        match rx.recv().await {
            Ok(ev) => {
                if !tm.keep_and_update(&ev) {
                    continue;
                }
                if let Err(e) = app_handle.emit("event", &ev) {
                    tracing::error!(error = %e, "failed to emit event to UI");
                }
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(dropped = n, "UI pusher lagged");
            }
            Err(RecvError::Closed) => return,
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct EventRow {
    pub id: i64,
    pub pid: i64,
    pub parent_pid: Option<i64>,
    pub ts: i64,
    pub kind: String,
    pub detail_json: String,
}

#[tauri::command]
pub async fn recent_events(
    pool: State<'_, SqlitePool>,
    limit: i64,
) -> Result<Vec<EventRow>, String> {
    sqlx::query_as::<_, EventRow>(
        "SELECT id, pid, parent_pid, ts, kind, detail_json \
         FROM events ORDER BY ts DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_findings(
    pool: State<'_, SqlitePool>,
    limit: i64,
    status: Option<String>,
) -> Result<Vec<Finding>, String> {
    findings::list_findings(pool.inner(), limit, status.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_finding_status(
    pool: State<'_, SqlitePool>,
    id: i64,
    new_status: String,
    notes: Option<String>,
) -> Result<(), String> {
    let status = FindingStatus::parse(&new_status)
        .ok_or_else(|| format!("invalid status: {new_status}"))?;
    findings::update_status(pool.inner(), id, status, notes.as_deref())
        .await
        .map_err(|e| e.to_string())
}

// ── Target CRUD ────────────────────────────────────

#[tauri::command]
pub async fn list_targets(
    tm: State<'_, Arc<TargetManager>>,
) -> Result<TargetStatus, String> {
    Ok(tm.snapshot_status())
}

#[tauri::command]
pub async fn add_target(
    tm: State<'_, Arc<TargetManager>>,
    name: String,
    match_kind: String,
    match_value: String,
) -> Result<i64, String> {
    let kind = TargetMatchKind::parse(&match_kind)
        .ok_or_else(|| format!("invalid match_kind: {match_kind}"))?;
    let t = Target {
        id: None,
        name,
        match_kind: kind,
        match_value,
        enabled: true,
        created_at: chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000),
    };
    tm.add(t).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_target(
    tm: State<'_, Arc<TargetManager>>,
    id: i64,
) -> Result<(), String> {
    tm.remove(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_target(
    tm: State<'_, Arc<TargetManager>>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    tm.toggle(id, enabled).await.map_err(|e| e.to_string())
}

// ── Dashboard ──────────────────────────────────────

#[tauri::command]
pub async fn dashboard_summary(
    pool: State<'_, SqlitePool>,
) -> Result<dashboard::DashboardSummary, String> {
    dashboard::summary(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

// ── Reports ────────────────────────────────────────

#[tauri::command]
pub async fn export_report(
    pool: State<'_, SqlitePool>,
    app: AppHandle,
    start_ts_ns: i64,
    end_ts_ns: i64,
    severity: Option<String>,
) -> Result<String, String> {
    use tauri::Manager;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolve app data dir: {e}"))?;
    let out_dir = data_dir.join("reports");
    let path = reports::generate_html_report(
        pool.inner(),
        &out_dir,
        start_ts_ns,
        end_ts_ns,
        severity.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// ── Settings / data retention ──────────────────────

#[tauri::command]
pub async fn get_data_stats(
    pool: State<'_, SqlitePool>,
    app: AppHandle,
) -> Result<settings::DataStats, String> {
    use tauri::Manager;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolve app data dir: {e}"))?;
    let db_path = data_dir.join("captain.sqlite");
    settings::data_stats(pool.inner(), &db_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_old_events(
    pool: State<'_, SqlitePool>,
    days: i64,
) -> Result<i64, String> {
    settings::clear_old_events(pool.inner(), days)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_all_data(pool: State<'_, SqlitePool>) -> Result<(), String> {
    settings::clear_all_data(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

// ── Rules CRUD ─────────────────────────────────────

#[tauri::command]
pub async fn list_all_rules(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<rule::loader::RuleWithSource>, String> {
    Ok(rule::loader::load_all_with_source(pool.inner()).await)
}

#[tauri::command]
pub async fn upsert_user_rule(
    pool: State<'_, SqlitePool>,
    tx: State<'_, RulesSender>,
    yaml_body: String,
) -> Result<(), String> {
    let rule: Rule = serde_yaml::from_str(&yaml_body)
        .map_err(|e| format!("YAML parse failed: {e}"))?;
    if rule.id.trim().is_empty() {
        return Err("rule id cannot be empty".into());
    }
    rule::store_rules::upsert(pool.inner(), &rule)
        .await
        .map_err(|e| e.to_string())?;
    // Refresh + push to engine.
    let fresh = rule::loader::load_all(pool.inner()).await;
    let _ = tx.send(fresh);
    Ok(())
}

#[tauri::command]
pub async fn delete_user_rule(
    pool: State<'_, SqlitePool>,
    tx: State<'_, RulesSender>,
    id: String,
) -> Result<(), String> {
    rule::store_rules::delete(pool.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;
    let fresh = rule::loader::load_all(pool.inner()).await;
    let _ = tx.send(fresh);
    Ok(())
}

#[tauri::command]
pub async fn set_rule_enabled(
    pool: State<'_, SqlitePool>,
    tx: State<'_, RulesSender>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    // For built-in rules, "disabling" means inserting a user-row with
    // enabled=false for the same id. The loader's merge then shadows
    // the built-in with the disabled user copy.
    let builtin_match = rule::loader::load_builtin().into_iter().find(|r| r.id == id);
    if let Some(mut b) = builtin_match {
        // If already in user table, just toggle that.
        let user = rule::store_rules::list_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
        if user.iter().any(|(u_id, _, _)| u_id == &id) {
            rule::store_rules::set_enabled(pool.inner(), &id, enabled)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            // Insert override row.
            b.enabled = enabled;
            rule::store_rules::upsert(pool.inner(), &b)
                .await
                .map_err(|e| e.to_string())?;
        }
    } else {
        // Pure user rule.
        rule::store_rules::set_enabled(pool.inner(), &id, enabled)
            .await
            .map_err(|e| e.to_string())?;
    }
    let fresh = rule::loader::load_all(pool.inner()).await;
    let _ = tx.send(fresh);
    Ok(())
}

// ── Status ─────────────────────────────────────────

#[derive(Serialize)]
pub struct Status {
    pub events_total: i64,
    pub findings_open: i64,
    pub findings_total: i64,
    pub helper_connected: bool,
    pub monitored_pid_count: usize,
    pub target_count: usize,
    pub mode: &'static str,
}

#[tauri::command]
pub async fn status(
    pool: State<'_, SqlitePool>,
    helper_connected: State<'_, Arc<AtomicBool>>,
    tm: State<'_, Arc<TargetManager>>,
) -> Result<Status, String> {
    let (events_total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM events")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let (findings_total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM findings")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let (findings_open,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM findings WHERE status = 'open'")
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    let ts = tm.snapshot_status();
    Ok(Status {
        events_total,
        findings_open,
        findings_total,
        helper_connected: helper_connected.load(Ordering::Relaxed),
        monitored_pid_count: ts.monitored_pid_count,
        target_count: ts.targets.len(),
        mode: ts.mode,
    })
}

#[allow(dead_code)]
fn _event_type_is_shared(_: Event) {}
