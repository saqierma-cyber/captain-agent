//! Generate the JSON config file passed to osqueryd.
//!
//! Slice 1: enable Endpoint Security + ES FIM. Subscribe to:
//!   - process_events     (all execve)
//!   - file_events        (sensitive paths defined under `captain_sensitive`)
//!   - socket_events      (outbound connects)

use anyhow::{Context, Result};
use serde_json::json;
use std::path::Path;

const SCHEDULE_INTERVAL_SEC: u64 = 5;

/// Sensitive file path globs. osquery uses `%` (single segment) and `%%`
/// (recursive). Tilde isn't expanded — we use `/Users/%/...` to match
/// anyone's home directory.
const FIM_SENSITIVE_PATHS: &[&str] = &[
    // Credentials
    "/Users/%/.ssh/%%",
    "/Users/%/.aws/%%",
    "/Users/%/.gnupg/%%",
    "/Users/%/.docker/config.json",
    "/Users/%/.netrc",
    "/Users/%/.kube/config",
    "/Users/%/.config/gh/%%",
    // Persistence (autostart)
    "/Users/%/Library/LaunchAgents/%%",
    "/Library/LaunchAgents/%%",
    "/Library/LaunchDaemons/%%",
];

pub async fn write_slice1_config(target_dir: &Path) -> Result<std::path::PathBuf> {
    tokio::fs::create_dir_all(target_dir)
        .await
        .with_context(|| format!("create config dir {}", target_dir.display()))?;

    let cfg = json!({
        "options": {
            "host_identifier": "captain-helper",
            "schedule_splay_percent": 10
        },
        "file_paths": {
            "captain_sensitive": FIM_SENSITIVE_PATHS
        },
        "schedule": {
            // NOTE: standard `process_events` on macOS 16 is silent — that
            // table was historically populated by OpenBSM (removed in
            // macOS 15+) and the ES backend feeds the macOS-specific
            // `es_process_events` table instead.
            //
            // We keep this query for cross-platform compat (Linux audit /
            // Windows ETW back the standard table) and rely on
            // `es_process_events` on macOS.
            "captain_process_events": {
                "query": "SELECT pid, parent, path, cmdline, uid, time \
                          FROM process_events;",
                "interval": SCHEDULE_INTERVAL_SEC,
                "snapshot": false,
                "removed": false
            },
            "captain_es_process_events": {
                "query": "SELECT pid, parent, path, cmdline, time \
                          FROM es_process_events;",
                "interval": SCHEDULE_INTERVAL_SEC,
                "snapshot": false,
                "removed": false
            },
            // NOTE: osquery's standard file_events table is path/inode-keyed
            // and does NOT include process PID. For Slice 1 we capture path +
            // action + time; PID attribution requires the macOS-only
            // es_process_file_events table (next query below). The two are
            // correlated by time + path at the rule-engine layer.
            "captain_file_events": {
                "query": "SELECT target_path AS path, action, uid AS owner_uid, time \
                          FROM file_events \
                          WHERE category = 'captain_sensitive';",
                "interval": SCHEDULE_INTERVAL_SEC,
                "snapshot": false,
                "removed": false
            },
            // macOS-only ES file events table — has PID attribution which
            // `file_events` (FIM) lacks. SQL-level path filter narrows the
            // firehose to ~10 sensitive prefixes. Column names confirmed
            // experimentally on osquery 5.23: pid, parent, path, time
            // (dest_path/operation/action don't exist in our build).
            //
            // We classify everything matching the sensitive paths as
            // FileRead — strictly this could be open/write/unlink but
            // having ANY PID-attributed event is what credential-read
            // rules need to fire on macOS 16 where FIM open events are
            // silently broken.
            "captain_es_process_file_events": {
                "query": "SELECT pid, parent, path, time \
                          FROM es_process_file_events \
                          WHERE (path LIKE '/Users/%/.ssh/%' \
                             OR  path LIKE '/Users/%/.aws/%' \
                             OR  path LIKE '/Users/%/.gnupg/%' \
                             OR  path LIKE '/Users/%/.kube/%' \
                             OR  path LIKE '/Users/%/.config/gh/%' \
                             OR  path LIKE '/Users/%/.docker/config.json' \
                             OR  path LIKE '/Users/%/.netrc' \
                             OR  path LIKE '/Users/%/Library/Keychains/%' \
                             OR  path LIKE '/Users/%/Library/LaunchAgents/%' \
                             OR  path LIKE '/Library/LaunchAgents/%' \
                             OR  path LIKE '/Library/LaunchDaemons/%');",
                "interval": SCHEDULE_INTERVAL_SEC,
                "snapshot": false,
                "removed": false
            },
            "captain_socket_events": {
                "query": "SELECT pid, family, protocol, remote_address, \
                                  remote_port, action, time \
                          FROM socket_events \
                          WHERE action IN ('connect', 'connected');",
                "interval": SCHEDULE_INTERVAL_SEC,
                "snapshot": false,
                "removed": false
            }
        }
    });

    let path = target_dir.join("osquery.conf");
    let body = serde_json::to_vec_pretty(&cfg)?;
    tokio::fs::write(&path, body)
        .await
        .with_context(|| format!("write osquery config to {}", path.display()))?;
    Ok(path)
}
