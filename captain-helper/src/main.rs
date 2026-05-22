//! captain-helper — the root LaunchDaemon.
//!
//! Spawns osqueryd with full Endpoint Security access, parses its stdout
//! JSON event stream, normalizes to the shared `Event` schema, and
//! broadcasts to UI clients over a Unix Domain Socket.
//!
//! The UDS server runs as a long-lived task. osqueryd runs in a separate
//! "supervisor loop" task that auto-restarts on crash with exponential
//! backoff capped at 30s. This way short osqueryd outages don't drop
//! UI clients — the socket stays bound, clients stay connected, events
//! resume as soon as osqueryd comes back.
//!
//! Paths (overridable via env vars for dev):
//!   CAPTAIN_OSQUERYD       path to osqueryd binary
//!                          default: /Library/Application Support/com.captainagent.helper/osquery.app/Contents/MacOS/osqueryd
//!   CAPTAIN_HELPER_SOCK    UDS path the server binds
//!                          default: /var/run/captain-helper.sock
//!   CAPTAIN_HELPER_STATE   state dir for osquery.conf, pidfile, rocksdb
//!                          default: /var/lib/captain-helper

mod bus;
mod osquery;
mod uds_server;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;

use crate::bus::Bus;
use crate::uds_server::SharedStatus;

const DEFAULT_OSQUERYD_PATH: &str =
    "/Library/Application Support/com.captainagent.helper/osquery.app/Contents/MacOS/osqueryd";
const DEFAULT_STATE_DIR: &str = "/var/lib/captain-helper";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("captain_helper=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let osqueryd_path = env_or(
        "CAPTAIN_OSQUERYD",
        PathBuf::from(DEFAULT_OSQUERYD_PATH),
    );
    let socket_path = env_or(
        "CAPTAIN_HELPER_SOCK",
        PathBuf::from(captain_common::DEFAULT_SOCKET_PATH),
    );
    let state_dir = env_or("CAPTAIN_HELPER_STATE", PathBuf::from(DEFAULT_STATE_DIR));

    tracing::info!(
        osqueryd = %osqueryd_path.display(),
        socket = %socket_path.display(),
        state = %state_dir.display(),
        "captain-helper booting"
    );

    if !osqueryd_path.exists() {
        anyhow::bail!(
            "osqueryd not found at {} (set CAPTAIN_OSQUERYD to override)",
            osqueryd_path.display()
        );
    }
    tokio::fs::create_dir_all(&state_dir)
        .await
        .with_context(|| format!("create state dir {}", state_dir.display()))?;

    let bus = Bus::new();
    let status = SharedStatus::new();

    // ── Supervisor loop: auto-restarts osqueryd on crash ───────────────
    let supervisor = Arc::new(osquery::Supervisor {
        osqueryd_path,
        state_dir: state_dir.clone(),
    });
    let supervisor_handle = tokio::spawn({
        let sup = supervisor.clone();
        let bus = bus.clone();
        let status = status.clone();
        async move {
            let mut backoff = Duration::from_millis(500);
            const MAX_BACKOFF: Duration = Duration::from_secs(30);
            loop {
                match sup.spawn(bus.clone(), status.clone()).await {
                    Ok(mut child) => {
                        let start = Instant::now();
                        let result = child.wait().await;
                        status.set_osqueryd_alive(false);
                        let ran_for = start.elapsed();
                        tracing::warn!(
                            ?result,
                            uptime_secs = ran_for.as_secs(),
                            "osqueryd exited — will restart in {:?}",
                            backoff
                        );
                        // If osqueryd ran for ≥ 60s before dying, treat as a
                        // healthy run and reset the backoff. Otherwise grow it.
                        if ran_for >= Duration::from_secs(60) {
                            backoff = Duration::from_millis(500);
                        } else {
                            backoff = (backoff * 2).min(MAX_BACKOFF);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "osqueryd spawn failed, retrying in {:?}", backoff);
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                    }
                }
                tokio::time::sleep(backoff).await;
            }
        }
    });

    // ── UDS server (independent of osqueryd) ────────────────────────────
    let server = uds_server::Server {
        socket_path: socket_path.clone(),
        bus: bus.clone(),
        status: status.clone(),
        started_at: Arc::new(Instant::now()),
    };
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.serve().await {
            tracing::error!(error = ?e, "UDS server crashed");
        }
    });

    let mut sigterm = signal(SignalKind::terminate()).context("install SIGTERM handler")?;
    let mut sigint = signal(SignalKind::interrupt()).context("install SIGINT handler")?;

    tokio::select! {
        _ = sigterm.recv() => tracing::info!("SIGTERM received"),
        _ = sigint.recv() => tracing::info!("SIGINT received"),
        _ = server_handle => tracing::warn!("UDS server task ended; shutting down"),
        // supervisor_handle's loop is infinite — it only "ends" if dropped.
        // We don't select on it here, intentionally: a crashed osqueryd
        // should not kill the daemon.
    }

    // Graceful shutdown: abort supervisor (kills any running osqueryd via
    // its kill_on_drop), then remove socket.
    supervisor_handle.abort();
    let _ = tokio::fs::remove_file(&socket_path).await;
    tracing::info!("captain-helper exited cleanly");
    Ok(())
}

fn env_or(key: &str, fallback: PathBuf) -> PathBuf {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => fallback,
    }
}

#[allow(dead_code)]
pub(crate) type SharedMutex<T> = Arc<Mutex<T>>;
