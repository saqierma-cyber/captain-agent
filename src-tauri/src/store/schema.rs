//! SQLite schema — 5 tables per spec.

use anyhow::{Context, Result};
use sqlx::SqlitePool;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS targets (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    match_kind  TEXT NOT NULL,
    match_value TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id         INTEGER PRIMARY KEY,
    target_id  INTEGER REFERENCES targets(id),
    root_pid   INTEGER NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER
);

CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY,
    session_id  INTEGER REFERENCES sessions(id),
    pid         INTEGER NOT NULL,
    parent_pid  INTEGER,
    ts          INTEGER NOT NULL,
    kind        TEXT NOT NULL,
    detail_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_session_ts ON events(session_id, ts);
CREATE INDEX IF NOT EXISTS idx_events_pid_ts ON events(pid, ts);
CREATE INDEX IF NOT EXISTS idx_events_kind_ts ON events(kind, ts);

CREATE TABLE IF NOT EXISTS findings (
    id            INTEGER PRIMARY KEY,
    event_id      INTEGER REFERENCES events(id),
    rule_id       TEXT NOT NULL,
    severity      TEXT NOT NULL,
    message       TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'open',
    notes         TEXT,
    created_at    INTEGER NOT NULL,
    pid           INTEGER NOT NULL DEFAULT 0,
    event_kind    TEXT NOT NULL DEFAULT '',
    event_summary TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_findings_severity_ts ON findings(severity, created_at);
CREATE INDEX IF NOT EXISTS idx_findings_status_ts ON findings(status, created_at);

CREATE TABLE IF NOT EXISTS rules (
    id         TEXT PRIMARY KEY,
    yaml_body  TEXT NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL
);
"#;

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    for stmt in SCHEMA_SQL.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(trimmed)
            .execute(pool)
            .await
            .with_context(|| format!("schema migration failed for statement: {trimmed}"))?;
    }

    // Compat migration for DBs created before Slice 2 added the
    // denormalized columns on `findings`. ALTER TABLE ADD COLUMN errors
    // with "duplicate column name" on fresh DBs — we swallow that.
    for (col, defn) in &[
        ("pid", "INTEGER NOT NULL DEFAULT 0"),
        ("event_kind", "TEXT NOT NULL DEFAULT ''"),
        ("event_summary", "TEXT NOT NULL DEFAULT ''"),
    ] {
        let sql = format!("ALTER TABLE findings ADD COLUMN {col} {defn}");
        if let Err(e) = sqlx::query(&sql).execute(pool).await {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                return Err(anyhow::anyhow!("ALTER findings ADD {col}: {msg}"));
            }
        }
    }
    Ok(())
}
