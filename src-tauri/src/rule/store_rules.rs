//! SQLite CRUD for user-defined rules.
//!
//! User rules are stored as raw YAML blobs. The `id` column is the
//! rule's id (so a user can override a built-in by adding a row with
//! the same id — the loader's merge puts user wins).

use anyhow::{Context, Result};
use captain_common::Rule;
use sqlx::SqlitePool;

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<(String, String, bool)>> {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT id, yaml_body, enabled FROM rules ORDER BY id")
            .fetch_all(pool)
            .await
            .context("list user rules")?;
    Ok(rows
        .into_iter()
        .map(|(id, body, en)| (id, body, en != 0))
        .collect())
}

/// Add or overwrite a user rule. We allow same id as a built-in so the
/// user can override it (e.g. disable a built-in by adding `enabled: false`).
pub async fn upsert(pool: &SqlitePool, rule: &Rule) -> Result<()> {
    let yaml_body = serde_yaml::to_string(rule).context("serialize rule")?;
    let now_ns = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000);
    sqlx::query(
        "INSERT INTO rules (id, yaml_body, enabled, updated_at) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(id) DO UPDATE SET \
           yaml_body = excluded.yaml_body, \
           enabled = excluded.enabled, \
           updated_at = excluded.updated_at",
    )
    .bind(&rule.id)
    .bind(&yaml_body)
    .bind(if rule.enabled { 1 } else { 0 })
    .bind(now_ns)
    .execute(pool)
    .await
    .context("upsert user rule")?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM rules WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .context("delete user rule")?;
    Ok(())
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> Result<()> {
    let now_ns = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000);
    sqlx::query("UPDATE rules SET enabled = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(if enabled { 1 } else { 0 })
        .bind(now_ns)
        .bind(id)
        .execute(pool)
        .await
        .context("toggle user rule")?;
    Ok(())
}
