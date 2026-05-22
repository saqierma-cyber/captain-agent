//! Core Event types — the protocol that ② Event Router writes,
//! ③ event bus carries, ④ rule engine reads, ⑤ store persists, ⑥ API emits.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum EventDetail {
    ProcessSpawn {
        exe: String,
        cmdline: String,
        uid: Option<i64>,
    },
    ProcessExit {
        exit_code: Option<i64>,
    },
    FileRead {
        path: String,
    },
    FileWrite {
        path: String,
    },
    FileDelete {
        path: String,
    },
    NetConnect {
        remote_addr: String,
        remote_port: u16,
        protocol: String,
    },
    DnsQuery {
        domain: String,
    },
    Persistence {
        path: String,
        action: String,
    },
}

impl EventDetail {
    pub fn kind_str(&self) -> &'static str {
        match self {
            EventDetail::ProcessSpawn { .. } => "process_spawn",
            EventDetail::ProcessExit { .. } => "process_exit",
            EventDetail::FileRead { .. } => "file_read",
            EventDetail::FileWrite { .. } => "file_write",
            EventDetail::FileDelete { .. } => "file_delete",
            EventDetail::NetConnect { .. } => "net_connect",
            EventDetail::DnsQuery { .. } => "dns_query",
            EventDetail::Persistence { .. } => "persistence",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Database ID. None for events still flowing through the bus / IPC.
    pub id: Option<i64>,
    /// Session ID — which monitored-app session this event belongs to.
    /// None until Slice 3 (Target Manager).
    pub session_id: Option<i64>,
    pub pid: i64,
    pub parent_pid: Option<i64>,
    /// Unix epoch nanoseconds.
    pub ts: i64,
    pub detail: EventDetail,
}

impl Event {
    pub fn new(pid: i64, parent_pid: Option<i64>, ts_ns: i64, detail: EventDetail) -> Self {
        Self {
            id: None,
            session_id: None,
            pid,
            parent_pid,
            ts: ts_ns,
            detail,
        }
    }
}
