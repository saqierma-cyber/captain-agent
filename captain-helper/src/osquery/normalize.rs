//! osquery JSON row → typed Event. Dispatch by `name` (which is the
//! scheduled-query name from osquery.conf).

use captain_common::{Event, EventDetail};
use serde_json::Value;

pub fn from_log_row(row: &Value) -> Option<Vec<Event>> {
    let name = row.get("name")?.as_str()?;
    let action = row.get("action")?.as_str().unwrap_or("");
    let columns = row.get("columns")?.as_object()?;

    let str_col = |k: &str| columns.get(k).and_then(|v| v.as_str()).map(|s| s.to_owned());
    let i64_col = |k: &str| {
        columns
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
    };

    // Prefer the per-row "time" column (osquery's view of when the syscall
    // happened); fall back to the log row's unixTime; fall back to now.
    let ts_sec = i64_col("time")
        .or_else(|| row.get("unixTime").and_then(|v| v.as_i64()))
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    let ts_ns = if ts_sec <= 0 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    } else {
        ts_sec.saturating_mul(1_000_000_000)
    };

    // Some tables (e.g. file_events) don't include PID. Use 0 as a sentinel
    // for "unknown process"; the rule engine can later correlate by time
    // + path with process_events / es_process_file_events.
    let pid = i64_col("pid").unwrap_or(0);
    let parent_pid = i64_col("parent");

    match name {
        // Both standard process_events (Linux/Windows back this) and
        // macOS-specific es_process_events map to ProcessSpawn. On Mac
        // 16 only the es_ variant produces rows; on other platforms only
        // the standard one. Same column shape works for both.
        //
        // ES on macOS emits a fork event (empty cmdline) followed by an
        // exec event (cmdline populated, if the process is still alive).
        // Drop the fork variant — keeping it would 2x our SQLite volume
        // AND flood rule evaluation with rows that can never match
        // pattern-based rules anyway.
        "captain_process_events" | "captain_es_process_events" => {
            let cmdline = str_col("cmdline").unwrap_or_default();
            if cmdline.trim().is_empty() {
                return None;
            }
            let exe = str_col("path").unwrap_or_default();
            let uid = i64_col("uid");
            Some(vec![Event::new(
                pid,
                parent_pid,
                ts_ns,
                EventDetail::ProcessSpawn { exe, cmdline, uid },
            )])
        }
        "captain_file_events" => {
            let path = str_col("path").unwrap_or_default();
            let act = action.to_ascii_lowercase();
            // osquery file_events actions: CREATED / UPDATED / OPENED /
            // RENAMED / ATTRIBUTES_MODIFIED / MOVED_TO / MOVED_FROM /
            // DELETED. Map to our 3 kinds.
            let detail = if act.contains("delete") || act == "moved_from" {
                EventDetail::FileDelete { path }
            } else if act.contains("opened") || act.contains("read") {
                EventDetail::FileRead { path }
            } else {
                EventDetail::FileWrite { path }
            };
            Some(vec![Event::new(pid, parent_pid, ts_ns, detail)])
        }
        // macOS-only ES file events — has PID attribution that file_events
        // lacks. SQL-level pre-filter in config_gen.rs narrows to
        // sensitive paths; everything that makes it here becomes FileRead.
        // Dedup window prevents duplicates if FIM also caught the same op.
        "captain_es_process_file_events" => {
            let path = str_col("path").unwrap_or_default();
            if path.is_empty() {
                return None;
            }
            Some(vec![Event::new(
                pid,
                parent_pid,
                ts_ns,
                EventDetail::FileRead { path },
            )])
        }
        "captain_socket_events" => {
            let remote_addr = str_col("remote_address").unwrap_or_default();
            let remote_port = str_col("remote_port")
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0);
            let proto = str_col("protocol").unwrap_or_default();
            let protocol = match proto.as_str() {
                "6" => "tcp".to_string(),
                "17" => "udp".to_string(),
                other => other.to_string(),
            };
            // Skip noisy localhost/IPC sockets to keep the timeline sane.
            if remote_addr.is_empty()
                || remote_addr.starts_with("127.")
                || remote_addr == "::1"
            {
                return None;
            }
            Some(vec![Event::new(
                pid,
                parent_pid,
                ts_ns,
                EventDetail::NetConnect {
                    remote_addr,
                    remote_port,
                    protocol,
                },
            )])
        }
        _ => None,
    }
}

// `is_sensitive_path` removed in Slice 4 — it was for es_process_file_events
// client-side filtering, which we disabled in Slice 1 pending Slice 5 schema
// introspection. When we re-enable that table (V2 milestone), the filter
// will move into config_gen.rs as a SQL WHERE clause instead.
