//! Async batched writer — consumes from the bus, accumulates events,
//! flushes to SQLite every 500ms or every 500 rows. Filters through
//! the TargetManager so non-monitored events are never persisted.

use crate::bus::Bus;
use crate::target::TargetManager;
use anyhow::Result;
use captain_common::Event;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::interval;

const BATCH_SIZE: usize = 500;
const FLUSH_INTERVAL_MS: u64 = 500;

pub async fn run(pool: SqlitePool, bus: Bus, tm: Arc<TargetManager>) -> Result<()> {
    let mut rx = bus.subscribe();
    let mut batch: Vec<Event> = Vec::with_capacity(BATCH_SIZE);
    let mut tick = interval(Duration::from_millis(FLUSH_INTERVAL_MS));

    loop {
        tokio::select! {
            recv = rx.recv() => match recv {
                Ok(ev) => {
                    if !tm.keep_and_update(&ev) {
                        continue;
                    }
                    batch.push(ev);
                    if batch.len() >= BATCH_SIZE {
                        flush(&pool, &mut batch).await?;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("store writer lagged, dropped {n} events");
                }
                Err(RecvError::Closed) => {
                    if !batch.is_empty() {
                        flush(&pool, &mut batch).await?;
                    }
                    tracing::info!("bus closed, store writer exiting");
                    return Ok(());
                }
            },
            _ = tick.tick() => {
                if !batch.is_empty() {
                    flush(&pool, &mut batch).await?;
                }
            }
        }
    }
}

async fn flush(pool: &SqlitePool, batch: &mut Vec<Event>) -> Result<()> {
    let mut tx = pool.begin().await?;
    for ev in batch.drain(..) {
        let detail_json = serde_json::to_string(&ev.detail)?;
        let kind = ev.detail.kind_str();
        sqlx::query(
            "INSERT INTO events (session_id, pid, parent_pid, ts, kind, detail_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(ev.session_id)
        .bind(ev.pid)
        .bind(ev.parent_pid)
        .bind(ev.ts)
        .bind(kind)
        .bind(detail_json)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
