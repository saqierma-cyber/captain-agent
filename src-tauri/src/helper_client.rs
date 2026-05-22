//! Helper client — connects to the captain-helper daemon's Unix domain
//! socket, subscribes to the event stream, and publishes incoming events
//! to the local Bus for store + UI consumers.
//!
//! Reconnect behavior: if the helper isn't running yet, retry with
//! exponential backoff up to 30s. Once connected, if we lose the link
//! mid-stream, retry the same way.

use crate::bus::Bus;
use anyhow::{Context, Result};
use captain_common::{ClientMessage, ServerMessage, DEFAULT_SOCKET_PATH};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Clone)]
pub struct HelperClient {
    pub socket_path: PathBuf,
    pub bus: Bus,
    /// Liveness flag the UI status badge reads.
    pub connected: Arc<AtomicBool>,
}

impl HelperClient {
    /// Resolve the socket path: `CAPTAIN_HELPER_SOCK` env override, or
    /// the shared default (`/var/run/captain-helper.sock`).
    pub fn resolve_socket() -> PathBuf {
        std::env::var("CAPTAIN_HELPER_SOCK")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH))
    }

    /// Long-running task. Returns when the bus is closed.
    pub async fn run(self) {
        let mut backoff_ms: u64 = 500;
        loop {
            self.connected.store(false, Ordering::Relaxed);
            match self.connect_and_stream().await {
                Ok(()) => {
                    tracing::info!("helper disconnected; reconnecting");
                    backoff_ms = 500;
                }
                Err(e) => {
                    tracing::debug!(error = %e, "helper connect/stream failed");
                }
            }
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(30_000);
        }
    }

    async fn connect_and_stream(&self) -> Result<()> {
        let stream = UnixStream::connect(&self.socket_path).await.with_context(|| {
            format!("connect to helper at {}", self.socket_path.display())
        })?;
        tracing::info!(socket = %self.socket_path.display(), "connected to helper");
        self.connected.store(true, Ordering::Relaxed);

        let (read_half, mut write_half) = stream.into_split();

        // Send Subscribe.
        let req = serde_json::to_vec(&ClientMessage::Subscribe)?;
        write_half.write_all(&req).await?;
        write_half.write_all(b"\n").await?;
        write_half.flush().await?;

        let mut reader = BufReader::new(read_half).lines();
        while let Some(line) = reader.next_line().await? {
            match serde_json::from_str::<ServerMessage>(&line) {
                Ok(ServerMessage::Event { event }) => {
                    self.bus.publish(event);
                }
                Ok(ServerMessage::Status(s)) => {
                    tracing::debug!(?s, "helper status");
                }
                Ok(ServerMessage::Pong) => {}
                Ok(ServerMessage::Error { message }) => {
                    tracing::warn!(%message, "helper error");
                }
                Ok(ServerMessage::AlivePids { .. }) => {
                    // The subscribe stream shouldn't normally see this; it's
                    // a reply to a one-shot RPC on a separate connection.
                    tracing::trace!("unexpected AlivePids on subscribe stream");
                }
                Err(e) => {
                    tracing::trace!(line = %line, error = %e, "malformed line from helper");
                }
            }
        }
        Ok(())
    }
}

/// One-shot RPC: open a fresh UDS connection, send ListAlivePids, read the
/// reply, close. Used by the periodic Target Manager GC task.
pub async fn fetch_alive_pids(socket_path: &std::path::Path) -> anyhow::Result<Vec<i64>> {
    use tokio::io::AsyncBufReadExt;
    let stream = UnixStream::connect(socket_path).await?;
    let (read_half, mut write_half) = stream.into_split();
    let req = serde_json::to_vec(&ClientMessage::ListAlivePids)?;
    write_half.write_all(&req).await?;
    write_half.write_all(b"\n").await?;
    write_half.flush().await?;

    let mut reader = tokio::io::BufReader::new(read_half).lines();
    if let Some(line) = reader.next_line().await? {
        let msg: ServerMessage = serde_json::from_str(&line)?;
        if let ServerMessage::AlivePids { pids } = msg {
            return Ok(pids);
        }
        anyhow::bail!("unexpected reply to ListAlivePids: {line}");
    }
    anyhow::bail!("connection closed before reply")
}
