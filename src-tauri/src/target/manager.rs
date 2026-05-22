//! ① Target Manager — stateful PID tracking + event filtering.
//!
//! Slice 3 model:
//! - State is in-memory (kept consistent with SQLite via reload on each
//!   write). Two maps: targets (id → Target) and monitored_pids (pid →
//!   target_id).
//! - On every ProcessSpawn event, we (a) check the new pid's exe against
//!   each enabled target's match pattern, (b) check whether its parent
//!   is already monitored (descendant tracking).
//! - keep_and_update returns true to let the event through, false to
//!   drop it. When no targets are configured, **always returns true** —
//!   the app is useful on day one before the user has configured anything.
//!
//! Process exit handling is deferred to Slice 4 — without ProcessExit
//! events the monitored set only grows. Acceptable for short dev/test
//! sessions; production will add `processes` snapshot reconciliation.

use anyhow::Result;
use captain_common::{Event, EventDetail, Target, TargetMatchKind};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::target::store_targets;

pub struct TargetManager {
    pool: SqlitePool,
    state: RwLock<TargetState>,
}

struct TargetState {
    targets: HashMap<i64, Target>,
    /// pid → target_id (which target this pid belongs to).
    monitored_pids: HashMap<i64, i64>,
}

#[derive(Serialize)]
pub struct TargetStatus {
    pub targets: Vec<Target>,
    pub monitored_pid_count: usize,
    pub mode: &'static str,
}

impl TargetManager {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        let targets = store_targets::list_all(&pool).await?;
        let mut by_id = HashMap::new();
        for t in targets {
            if let Some(id) = t.id {
                by_id.insert(id, t);
            }
        }
        Ok(Self {
            pool,
            state: RwLock::new(TargetState {
                targets: by_id,
                monitored_pids: HashMap::new(),
            }),
        })
    }

    /// Per-event hook called by every bus consumer. Returns whether the
    /// event should be processed downstream. Also has the side effect of
    /// updating the monitored PID set on ProcessSpawn.
    pub fn keep_and_update(&self, ev: &Event) -> bool {
        // Update phase: ProcessSpawn might add a new monitored PID.
        if let EventDetail::ProcessSpawn { exe, .. } = &ev.detail {
            self.maybe_add_pid(ev.pid, ev.parent_pid, exe);
        }
        // Filter phase.
        let s = self.state.read().unwrap();
        if s.targets.values().all(|t| !t.enabled) {
            // No enabled targets ⇒ "show all" fallback so the UI is
            // useful before/without configuration.
            return true;
        }
        // pid=0 sentinel = events with no PID attribution (today: FIM
        // file_events on macOS). Filter mode can't decide if they belong
        // to a monitored target, so we let them through unconditionally —
        // better to over-show than to hide important file activity.
        // Slice 5 fix: switch to es_process_file_events table for real PID.
        if ev.pid == 0 {
            return true;
        }
        s.monitored_pids.contains_key(&ev.pid)
    }

    fn maybe_add_pid(&self, pid: i64, parent_pid: Option<i64>, exe: &str) {
        // Direct match against enabled targets.
        let direct: Option<i64> = {
            let s = self.state.read().unwrap();
            s.targets
                .values()
                .filter(|t| t.enabled)
                .find(|t| matches(t, exe))
                .and_then(|t| t.id)
        };
        if let Some(target_id) = direct {
            self.state
                .write()
                .unwrap()
                .monitored_pids
                .insert(pid, target_id);
            return;
        }
        // Descendant: if parent is monitored, this pid inherits.
        if let Some(ppid) = parent_pid {
            let inherited: Option<i64> = {
                let s = self.state.read().unwrap();
                s.monitored_pids.get(&ppid).copied()
            };
            if let Some(target_id) = inherited {
                self.state
                    .write()
                    .unwrap()
                    .monitored_pids
                    .insert(pid, target_id);
            }
        }
    }

    pub fn snapshot_status(&self) -> TargetStatus {
        let s = self.state.read().unwrap();
        let mut targets: Vec<Target> = s.targets.values().cloned().collect();
        targets.sort_by_key(|t| t.id.unwrap_or(0));
        let mode = if s.targets.values().any(|t| t.enabled) {
            "filter"
        } else {
            "show_all"
        };
        TargetStatus {
            targets,
            monitored_pid_count: s.monitored_pids.len(),
            mode,
        }
    }

    pub async fn add(&self, t: Target) -> Result<i64> {
        let id = store_targets::insert(&self.pool, &t).await?;
        let mut s = self.state.write().unwrap();
        let mut stored = t;
        stored.id = Some(id);
        s.targets.insert(id, stored);
        Ok(id)
    }

    pub async fn remove(&self, id: i64) -> Result<()> {
        store_targets::delete(&self.pool, id).await?;
        let mut s = self.state.write().unwrap();
        s.targets.remove(&id);
        // Drop any monitored PIDs that belonged exclusively to this target.
        s.monitored_pids.retain(|_, tid| *tid != id);
        Ok(())
    }

    pub async fn toggle(&self, id: i64, enabled: bool) -> Result<()> {
        store_targets::set_enabled(&self.pool, id, enabled).await?;
        let mut s = self.state.write().unwrap();
        if let Some(t) = s.targets.get_mut(&id) {
            t.enabled = enabled;
        }
        if !enabled {
            s.monitored_pids.retain(|_, tid| *tid != id);
        }
        Ok(())
    }

    /// Drop any monitored PID that's no longer alive. Called periodically
    /// (every 30s) by the GC task that polls captain-helper for the
    /// current alive-PID list. Returns how many were pruned.
    pub fn prune_dead(&self, alive: &std::collections::HashSet<i64>) -> usize {
        let mut s = self.state.write().unwrap();
        let before = s.monitored_pids.len();
        s.monitored_pids.retain(|pid, _| alive.contains(pid));
        before - s.monitored_pids.len()
    }
}

fn matches(t: &Target, exe: &str) -> bool {
    match t.match_kind {
        TargetMatchKind::ExePath => exe == t.match_value,
        TargetMatchKind::ExePrefix => exe.starts_with(&t.match_value),
        TargetMatchKind::ProcessName => std::path::Path::new(exe)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case(&t.match_value))
            .unwrap_or(false),
        TargetMatchKind::BundleId => false, // V2
    }
}
