//! Unix Domain Socket server. Listens for UI clients, broadcasts events
//! and responds to control messages.

use crate::bus::Bus;
use anyhow::{Context, Result};
use captain_common::{ClientMessage, HelperStatus, ServerMessage};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast::error::RecvError;

/// Process-wide counters / liveness, shared between supervisor and UDS server.
#[derive(Clone)]
pub struct SharedStatus {
    osqueryd_alive: Arc<AtomicBool>,
    events_emitted_total: Arc<AtomicU64>,
}

impl SharedStatus {
    pub fn new() -> Self {
        Self {
            osqueryd_alive: Arc::new(AtomicBool::new(false)),
            events_emitted_total: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_osqueryd_alive(&self, alive: bool) {
        self.osqueryd_alive.store(alive, Ordering::Relaxed);
    }

    pub fn osqueryd_alive(&self) -> bool {
        self.osqueryd_alive.load(Ordering::Relaxed)
    }

    pub fn bump_events_emitted(&self) {
        self.events_emitted_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn events_emitted_total(&self) -> u64 {
        self.events_emitted_total.load(Ordering::Relaxed)
    }
}

impl Default for SharedStatus {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Server {
    pub socket_path: PathBuf,
    pub bus: Bus,
    pub status: SharedStatus,
    pub started_at: Arc<Instant>,
}

impl Server {
    pub async fn serve(&self) -> Result<()> {
        // Remove a stale socket file from a previous crash; bind fresh.
        let _ = tokio::fs::remove_file(&self.socket_path).await;
        // Ensure parent dir exists.
        if let Some(parent) = self.socket_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        let listener = UnixListener::bind(&self.socket_path)
            .with_context(|| format!("bind UDS {}", self.socket_path.display()))?;

        // chmod 0666 so the user-mode UI can connect to a root-owned socket.
        let perms = std::fs::Permissions::from_mode(0o666);
        std::fs::set_permissions(&self.socket_path, perms)
            .with_context(|| format!("chmod 0666 {}", self.socket_path.display()))?;

        tracing::info!(socket = %self.socket_path.display(), "UDS server listening");

        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "UDS accept error");
                    continue;
                }
            };
            let bus = self.bus.clone();
            let status = self.status.clone();
            let started_at = self.started_at.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_client(stream, bus, status, started_at).await {
                    tracing::warn!(error = %e, "client task ended");
                }
            });
        }
    }
}

async fn handle_client(
    stream: UnixStream,
    bus: Bus,
    status: SharedStatus,
    started_at: Arc<Instant>,
) -> Result<()> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half).lines();
    let mut writer = write_half;

    while let Some(line) = reader.next_line().await? {
        let msg: ClientMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(line = %line, error = %e, "malformed client message");
                write_msg(
                    &mut writer,
                    &ServerMessage::Error {
                        message: format!("malformed JSON: {e}"),
                    },
                )
                .await?;
                continue;
            }
        };

        match msg {
            ClientMessage::Ping => {
                write_msg(&mut writer, &ServerMessage::Pong).await?;
            }
            ClientMessage::GetStatus => {
                let reply = ServerMessage::Status(HelperStatus {
                    osqueryd_alive: status.osqueryd_alive(),
                    osqueryd_version: None,
                    helper_version: env!("CARGO_PKG_VERSION").to_string(),
                    events_emitted_total: status.events_emitted_total(),
                    uptime_seconds: started_at.elapsed().as_secs(),
                });
                write_msg(&mut writer, &reply).await?;
            }
            ClientMessage::Subscribe => {
                tracing::debug!("client subscribed to event stream");
                let mut rx = bus.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            let msg = ServerMessage::Event { event: ev };
                            if write_msg(&mut writer, &msg).await.is_err() {
                                tracing::debug!("client disconnected mid-stream");
                                return Ok(());
                            }
                        }
                        Err(RecvError::Lagged(n)) => {
                            tracing::warn!(dropped = n, "client lagged");
                        }
                        Err(RecvError::Closed) => return Ok(()),
                    }
                }
            }
            ClientMessage::ListAlivePids => {
                let pids = tokio::task::spawn_blocking(list_alive_pids).await?;
                write_msg(&mut writer, &ServerMessage::AlivePids { pids }).await?;
            }
        }
    }
    Ok(())
}

/// Return current alive PIDs on this host. Uses `sysinfo` so we don't need
/// to round-trip through osquery for this hot-path call.
fn list_alive_pids() -> Vec<i64> {
    use sysinfo::{ProcessRefreshKind, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessRefreshKind::new());
    sys.processes()
        .keys()
        .map(|pid| pid.as_u32() as i64)
        .collect()
}

async fn write_msg<W: AsyncWriteExt + Unpin>(w: &mut W, msg: &ServerMessage) -> Result<()> {
    let mut line = serde_json::to_vec(msg)?;
    line.push(b'\n');
    w.write_all(&line).await?;
    w.flush().await?;
    Ok(())
}
