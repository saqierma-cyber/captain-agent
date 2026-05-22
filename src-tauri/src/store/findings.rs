//! Findings CRUD helpers — insert from rule engine; query + update from API.

use anyhow::{Context, Result};
use captain_common::{Finding, FindingStatus, Severity};
use sqlx::SqlitePool;
use sqlx::Row;

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
        Severity::Critical => "critical",
    }
}

fn parse_severity(s: &str) -> Severity {
    match s {
        "info" => Severity::Info,
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => Severity::Medium,
    }
}

pub async fn insert_finding(pool: &SqlitePool, f: &Finding) -> Result<i64> {
    let row = sqlx::query(
        "INSERT INTO findings \
           (event_id, rule_id, severity, message, status, notes, created_at, \
            pid, event_kind, event_summary) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
         RETURNING id",
    )
    .bind(f.event_id)
    .bind(&f.rule_id)
    .bind(severity_str(f.severity))
    .bind(&f.message)
    .bind(f.status.as_str())
    .bind(&f.notes)
    .bind(f.created_at)
    .bind(f.pid)
    .bind(&f.event_kind)
    .bind(&f.event_summary)
    .fetch_one(pool)
    .await
    .context("insert finding")?;
    Ok(row.get::<i64, _>("id"))
}

pub async fn list_findings(
    pool: &SqlitePool,
    limit: i64,
    status: Option<&str>,
) -> Result<Vec<Finding>> {
    let rows = if let Some(s) = status {
        sqlx::query(
            "SELECT id, event_id, rule_id, severity, message, status, notes, created_at, \
                    pid, event_kind, event_summary \
             FROM findings WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(s)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, event_id, rule_id, severity, message, status, notes, created_at, \
                    pid, event_kind, event_summary \
             FROM findings ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .context("list findings")?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let status_str: String = r.get("status");
        out.push(Finding {
            id: Some(r.get::<i64, _>("id")),
            event_id: r.try_get::<Option<i64>, _>("event_id").unwrap_or(None),
            rule_id: r.get::<String, _>("rule_id"),
            severity: parse_severity(&r.get::<String, _>("severity")),
            message: r.get::<String, _>("message"),
            status: FindingStatus::parse(&status_str).unwrap_or(FindingStatus::Open),
            notes: r.try_get::<Option<String>, _>("notes").unwrap_or(None),
            created_at: r.get::<i64, _>("created_at"),
            pid: r.get::<i64, _>("pid"),
            event_kind: r.get::<String, _>("event_kind"),
            event_summary: r.get::<String, _>("event_summary"),
        });
    }
    Ok(out)
}

pub async fn update_status(
    pool: &SqlitePool,
    id: i64,
    new_status: FindingStatus,
    notes: Option<&str>,
) -> Result<()> {
    sqlx::query("UPDATE findings SET status = ?1, notes = ?2 WHERE id = ?3")
        .bind(new_status.as_str())
        .bind(notes)
        .bind(id)
        .execute(pool)
        .await
        .context("update finding status")?;
    Ok(())
}
