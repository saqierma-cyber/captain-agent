//! ⑤ SQLite storage — schema + async batch writer.
//!
//! WAL mode + batched transactions so the high-frequency event stream
//! doesn't fsync our CPU into the ground.

pub mod batch_writer;
pub mod findings;
pub mod schema;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

/// Open the captain-agent SQLite database at `path`, applying schema migrations.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let options = SqliteConnectOptions::from_str(&url)
        .with_context(|| format!("invalid sqlite URL: {url}"))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .context("failed to open SQLite pool")?;

    schema::migrate(&pool).await?;
    Ok(pool)
}
