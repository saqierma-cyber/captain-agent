//! Dashboard aggregate queries — KPI counters, severity distribution,
//! top firing rules, time-bucketed event histogram.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{Row, SqlitePool};

#[derive(Serialize)]
pub struct DashboardSummary {
    pub total_events: i64,
    pub total_findings: i64,
    pub events_last_hour: i64,
    pub events_last_24h: i64,
    pub findings_last_hour: i64,
    pub findings_last_24h: i64,
    pub by_severity: Vec<SeverityBucket>,
    pub top_rules: Vec<TopRule>,
    pub events_by_hour_24h: Vec<HourBucket>,
    pub events_by_kind_24h: Vec<KindBucket>,
}

#[derive(Serialize)]
pub struct SeverityBucket {
    pub severity: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct TopRule {
    pub rule_id: String,
    pub severity: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct HourBucket {
    /// Unix epoch SECONDS of the hour's start.
    pub hour_start: i64,
    pub count: i64,
}

#[derive(Serialize)]
pub struct KindBucket {
    pub kind: String,
    pub count: i64,
}

pub async fn summary(pool: &SqlitePool) -> Result<DashboardSummary> {
    let now_ns = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000);
    let hour_ago = now_ns - 3_600_000_000_000i64;
    let day_ago = now_ns - 86_400_000_000_000i64;

    let total_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(pool)
        .await
        .context("total events")?;
    let total_findings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM findings")
        .fetch_one(pool)
        .await
        .context("total findings")?;
    let events_last_hour: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE ts > ?1")
            .bind(hour_ago)
            .fetch_one(pool)
            .await
            .context("events last hour")?;
    let events_last_24h: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE ts > ?1")
            .bind(day_ago)
            .fetch_one(pool)
            .await
            .context("events last 24h")?;
    let findings_last_hour: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM findings WHERE created_at > ?1")
            .bind(hour_ago)
            .fetch_one(pool)
            .await
            .context("findings last hour")?;
    let findings_last_24h: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM findings WHERE created_at > ?1")
            .bind(day_ago)
            .fetch_one(pool)
            .await
            .context("findings last 24h")?;

    let by_severity_rows = sqlx::query(
        "SELECT severity, COUNT(*) AS n FROM findings GROUP BY severity ORDER BY \
         CASE severity \
           WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 \
           WHEN 'low' THEN 3 ELSE 4 END",
    )
    .fetch_all(pool)
    .await
    .context("findings by severity")?;
    let by_severity: Vec<SeverityBucket> = by_severity_rows
        .iter()
        .map(|r| SeverityBucket {
            severity: r.get("severity"),
            count: r.get("n"),
        })
        .collect();

    let top_rule_rows = sqlx::query(
        "SELECT rule_id, severity, COUNT(*) AS n FROM findings \
         GROUP BY rule_id ORDER BY n DESC LIMIT 10",
    )
    .fetch_all(pool)
    .await
    .context("top rules")?;
    let top_rules: Vec<TopRule> = top_rule_rows
        .iter()
        .map(|r| TopRule {
            rule_id: r.get("rule_id"),
            severity: r.get("severity"),
            count: r.get("n"),
        })
        .collect();

    // 24 hourly buckets. Each bucket: events with ts in [start, start+1h).
    // Bucket start = floor(ts_seconds / 3600) * 3600.
    let hour_bucket_rows = sqlx::query(
        "SELECT (ts / 3600000000000) * 3600 AS hour_start, COUNT(*) AS n \
         FROM events WHERE ts > ?1 GROUP BY hour_start ORDER BY hour_start",
    )
    .bind(day_ago)
    .fetch_all(pool)
    .await
    .context("events by hour")?;
    let events_by_hour_24h: Vec<HourBucket> = hour_bucket_rows
        .iter()
        .map(|r| HourBucket {
            hour_start: r.get("hour_start"),
            count: r.get("n"),
        })
        .collect();

    let kind_rows = sqlx::query(
        "SELECT kind, COUNT(*) AS n FROM events WHERE ts > ?1 \
         GROUP BY kind ORDER BY n DESC",
    )
    .bind(day_ago)
    .fetch_all(pool)
    .await
    .context("events by kind")?;
    let events_by_kind_24h: Vec<KindBucket> = kind_rows
        .iter()
        .map(|r| KindBucket {
            kind: r.get("kind"),
            count: r.get("n"),
        })
        .collect();

    Ok(DashboardSummary {
        total_events,
        total_findings,
        events_last_hour,
        events_last_24h,
        findings_last_hour,
        findings_last_24h,
        by_severity,
        top_rules,
        events_by_hour_24h,
        events_by_kind_24h,
    })
}
