//! SQLite CRUD for the `targets` table.

use anyhow::{Context, Result};
use captain_common::{Target, TargetMatchKind};
use sqlx::{Row, SqlitePool};

fn row_to_target(r: &sqlx::sqlite::SqliteRow) -> Result<Target> {
    let id: i64 = r.try_get("id")?;
    let name: String = r.try_get("name")?;
    let kind_s: String = r.try_get("match_kind")?;
    let kind = TargetMatchKind::parse(&kind_s)
        .ok_or_else(|| anyhow::anyhow!("invalid match_kind in DB: {kind_s}"))?;
    let match_value: String = r.try_get("match_value")?;
    let enabled_i: i64 = r.try_get("enabled")?;
    let created_at: i64 = r.try_get("created_at")?;
    Ok(Target {
        id: Some(id),
        name,
        match_kind: kind,
        match_value,
        enabled: enabled_i != 0,
        created_at,
    })
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Target>> {
    let rows = sqlx::query(
        "SELECT id, name, match_kind, match_value, enabled, created_at \
         FROM targets ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .context("list targets")?;
    rows.iter().map(row_to_target).collect()
}

pub async fn insert(pool: &SqlitePool, t: &Target) -> Result<i64> {
    let r = sqlx::query(
        "INSERT INTO targets (name, match_kind, match_value, enabled, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
    )
    .bind(&t.name)
    .bind(t.match_kind.as_str())
    .bind(&t.match_value)
    .bind(if t.enabled { 1 } else { 0 })
    .bind(t.created_at)
    .fetch_one(pool)
    .await
    .context("insert target")?;
    Ok(r.get::<i64, _>("id"))
}

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM targets WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .context("delete target")?;
    Ok(())
}

pub async fn set_enabled(pool: &SqlitePool, id: i64, enabled: bool) -> Result<()> {
    sqlx::query("UPDATE targets SET enabled = ?1 WHERE id = ?2")
        .bind(if enabled { 1 } else { 0 })
        .bind(id)
        .execute(pool)
        .await
        .context("toggle target enabled")?;
    Ok(())
}
