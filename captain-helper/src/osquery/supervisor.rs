//! Spawn osqueryd with Endpoint Security + ES FIM enabled, pipe stdout
//! and stderr, hand them to the subscriber task.

use crate::bus::Bus;
use crate::osquery::{config_gen, subscriber};
use crate::uds_server::SharedStatus;
use anyhow::{anyhow, Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::{Child, Command};

pub struct Supervisor {
    pub osqueryd_path: PathBuf,
    pub state_dir: PathBuf,
}

impl Supervisor {
    pub async fn spawn(&self, bus: Bus, status: SharedStatus) -> Result<Child> {
        if !self.osqueryd_path.exists() {
            return Err(anyhow!(
                "osqueryd not found at {}",
                self.osqueryd_path.display()
            ));
        }
        tokio::fs::create_dir_all(&self.state_dir)
            .await
            .with_context(|| format!("create state dir {}", self.state_dir.display()))?;

        let config_path = config_gen::write_slice1_config(&self.state_dir).await?;
        let pidfile = self.state_dir.join("osqueryd.pid");
        let db_path = self.state_dir.join("osquery.db");

        // Detect config-vs-rocksdb mismatch and wipe state on change.
        // osquery persists scheduled-query state across runs; when we
        // remove or rename a query in our config, the stale entry stays
        // until the rocksdb is wiped, causing the "no such column" loops
        // we saw in Slices 2 + 4 dev. Hash the config and only wipe when
        // it actually differs from what's recorded.
        if let Err(e) = wipe_rocksdb_if_config_changed(&self.state_dir, &config_path).await {
            tracing::warn!(error = %e, "rocksdb consistency check failed (continuing)");
        }

        tracing::info!(
            osqueryd = %self.osqueryd_path.display(),
            config = %config_path.display(),
            "spawning osqueryd (root mode: ES + FIM)"
        );

        let mut cmd = Command::new(&self.osqueryd_path);
        cmd.arg(format!("--config_path={}", config_path.display()))
            .arg(format!("--pidfile={}", pidfile.display()))
            .arg(format!("--database_path={}", db_path.display()))
            // CLI-only flags
            .arg("--logger_plugin=stdout")
            .arg("--disable_events=false")
            .arg("--disable_endpointsecurity=false")
            .arg("--disable_endpointsecurity_fim=false")
            .arg("--enable_file_events=true")
            // FIM by default only emits CREATE / WRITE / DELETE.
            // Reads (which we need for ssh-key / aws-creds rules) require
            // explicit opt-in because they 10x the event volume.
            .arg("--es_fim_enable_open_events=true")
            .arg("--disable_extensions=true")
            .arg("--disable_watchdog=true")
            .arg("--force=true")
            .arg("--ephemeral=false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn().context("failed to spawn osqueryd")?;
        status.set_osqueryd_alive(true);

        if let Some(stdout) = child.stdout.take() {
            let bus = bus.clone();
            let status = status.clone();
            tokio::spawn(async move {
                if let Err(e) = subscriber::run(stdout, bus, status).await {
                    tracing::error!(error = %e, "osquery subscriber error");
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Up at INFO so LaunchDaemon log captures osqueryd
                    // diagnostics ("Event publisher not enabled: ...",
                    // "EndpointSecurity client cannot be created", etc.)
                    // without needing RUST_LOG tweaks.
                    tracing::info!(target: "osqueryd", "{}", line);
                }
            });
        }

        Ok(child)
    }
}

/// If the current osquery.conf differs from the hash we stored last time,
/// wipe osqueryd's rocksdb so it forgets all stale scheduled-query state.
/// Cheap: a single hash compare + a file write per restart.
async fn wipe_rocksdb_if_config_changed(state_dir: &Path, config_path: &Path) -> Result<()> {
    let conf_bytes = tokio::fs::read(config_path)
        .await
        .with_context(|| format!("read {} for hash", config_path.display()))?;
    let mut h = DefaultHasher::new();
    h.write(&conf_bytes);
    let current = format!("{:016x}", h.finish());

    let hash_file = state_dir.join("schedule.hash");
    let previous = tokio::fs::read_to_string(&hash_file).await.ok();

    if previous.as_deref().map(str::trim) == Some(current.trim()) {
        return Ok(());
    }

    tracing::info!(
        previous = previous.as_deref().unwrap_or("<none>"),
        current = %current,
        "config schedule changed — wiping osquery rocksdb"
    );

    let db_base = state_dir.join("osquery.db");
    for suffix in ["", ".dir", "-shm", "-wal"] {
        let p = if suffix.is_empty() {
            db_base.clone()
        } else {
            state_dir.join(format!("osquery.db{suffix}"))
        };
        if p.exists() {
            if p.is_dir() {
                let _ = tokio::fs::remove_dir_all(&p).await;
            } else {
                let _ = tokio::fs::remove_file(&p).await;
            }
        }
    }
    let _ = tokio::fs::remove_file(state_dir.join("osqueryd.pid")).await;

    tokio::fs::write(&hash_file, current).await.ok();
    Ok(())
}
