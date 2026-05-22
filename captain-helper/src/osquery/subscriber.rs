//! Read osqueryd stdout line-by-line, parse JSON, normalize, publish
//! to the helper's Bus.

use crate::bus::Bus;
use crate::osquery::normalize;
use crate::uds_server::SharedStatus;
use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

pub async fn run<R: AsyncRead + Unpin>(
    stdout: R,
    bus: Bus,
    status: SharedStatus,
) -> Result<()> {
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(row) => {
                if let Some(events) = normalize::from_log_row(&row) {
                    for ev in events {
                        bus.publish(ev);
                        status.bump_events_emitted();
                    }
                }
            }
            Err(_) => {
                // osqueryd writes diagnostic status lines to stdout
                // alongside the JSON event log (they look like
                // `severity=0 location=foo.cpp message=...`). Surface
                // them so ES/TCC failures are visible in the daemon log.
                tracing::info!(target: "osqueryd_stdout", "{}", line);
            }
        }
    }
    tracing::info!("osquery stdout closed");
    Ok(())
}
