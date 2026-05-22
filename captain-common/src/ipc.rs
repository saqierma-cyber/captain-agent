//! IPC protocol between `captain-helper` (root daemon, the server) and
//! `captain-agent` UI (user, the client).
//!
//! ## Transport
//! Unix Domain Socket. The helper binds and listens; the UI connects.
//! Default socket path: [`DEFAULT_SOCKET_PATH`]. Helper chmod's the socket
//! to 0666 so any local user can connect (auth comes in a later slice).
//!
//! ## Framing
//! JSON Lines: one JSON object per line, newline-delimited. Chosen for
//! ease of debugging — you can `sudo nc -U /var/run/captain-helper.sock`
//! and read the stream by eye. Event volume (tens to hundreds/sec) is
//! well within JSON's perf envelope.
//!
//! ## Handshake
//! Client connects → sends one [`ClientMessage::Subscribe`] →
//! server starts streaming [`ServerMessage::Event`] until client disconnects.
//! Other commands ([`ClientMessage::GetStatus`], [`ClientMessage::Ping`])
//! get a one-shot reply.

use crate::event::Event;
use serde::{Deserialize, Serialize};

/// Default UDS path. Helper writes here as root.
pub const DEFAULT_SOCKET_PATH: &str = "/var/run/captain-helper.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to the live event stream. Server keeps pushing
    /// `ServerMessage::Event` until the client disconnects.
    Subscribe,
    /// One-shot request for helper status.
    GetStatus,
    /// Liveness probe.
    Ping,
    /// One-shot request for current set of alive PIDs on the system.
    /// Used by the Tauri Target Manager to GC its monitored set when
    /// processes exit (we don't yet have ProcessExit events flowing).
    ListAlivePids,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// A normalized event from osquery.
    Event { event: Event },
    /// Reply to GetStatus.
    Status(HelperStatus),
    /// Reply to Ping.
    Pong,
    /// Helper-side error message that the UI should surface.
    Error { message: String },
    /// Reply to ListAlivePids.
    AlivePids { pids: Vec<i64> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperStatus {
    pub osqueryd_alive: bool,
    pub osqueryd_version: Option<String>,
    pub helper_version: String,
    pub events_emitted_total: u64,
    /// Seconds since helper started.
    pub uptime_seconds: u64,
}
