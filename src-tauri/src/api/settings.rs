//! Settings / data retention API.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Serialize)]
pub struct DataStats {
    /// Size of the SQLite file in bytes.
    pub db_bytes: u64,
    pub events_total: i64,
    pub findings_total: i64,
    /// Unix epoch nanoseconds of the oldest event we still hold.
    pub oldest_event_ts: Option<i64>,
}

pub async fn data_stats(pool: &SqlitePool, db_path: &std::path::Path) -> Result<DataStats> {
    let db_bytes = tokio::fs::metadata(db_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let events_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(pool)
        .await?;
    let findings_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM findings")
        .fetch_one(pool)
        .await?;
    // Filter out the ts<=0 sentinels from Slice 0's polling era. We only
    // want a meaningful "oldest real timestamp" for the UI display.
    let oldest_event_ts: Option<i64> =
        sqlx::query_scalar("SELECT MIN(ts) FROM events WHERE ts > 0")
            .fetch_one(pool)
            .await
            .unwrap_or(None);
    Ok(DataStats {
        db_bytes,
        events_total,
        findings_total,
        oldest_event_ts,
    })
}

/// Delete events older than `days` days. Findings are NOT deleted by this
/// (they're the curated layer; user explicitly cleans them via UI).
pub async fn clear_old_events(pool: &SqlitePool, days: i64) -> Result<i64> {
    let cutoff_ns = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000)
        - days * 86_400_000_000_000i64;
    let r = sqlx::query("DELETE FROM events WHERE ts < ?1")
        .bind(cutoff_ns)
        .execute(pool)
        .await
        .context("delete old events")?;
    Ok(r.rows_affected() as i64)
}

/// Wipe all events + findings (but keep targets + user rules).
pub async fn clear_all_data(pool: &SqlitePool) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM findings").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM events").execute(&mut *tx).await?;
    tx.commit().await?;
    // VACUUM has to run outside a transaction.
    sqlx::query("VACUUM").execute(pool).await.ok();
    Ok(())
}
