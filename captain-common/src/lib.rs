//! Captain Agent — shared types between the user-mode Tauri UI and the
//! root-mode `captain-helper` daemon.
//!
//! Five modules:
//! - [`event`]: the typed Event protocol (persisted to SQLite + Bus)
//! - [`ipc`]: JSON-Lines-over-UDS protocol between helper and UI
//! - [`rule`]: rule definitions (loaded from YAML, evaluated by engine)
//! - [`finding`]: what the rule engine emits when a rule matches an event
//! - [`target`]: a monitored app — exe path / bundle id / process name

pub mod event;
pub mod finding;
pub mod ipc;
pub mod rule;
pub mod target;

pub use event::{Event, EventDetail, Severity};
pub use finding::{Finding, FindingStatus};
pub use ipc::{ClientMessage, DEFAULT_SOCKET_PATH, HelperStatus, ServerMessage};
pub use rule::{Rule, RulePack, RuleType};
pub use target::{Target, TargetMatchKind};
