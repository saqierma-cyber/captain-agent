//! Load rules: built-in pack from compile-time YAMLs + user rules
//! from the SQLite `rules` table.

use crate::rule::builtin;
use captain_common::{Rule, RulePack};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// One-shot helper: load builtin + user rules and merge. Used by lib.rs
/// at boot and by Rules CRUD commands to refresh the watch channel.
pub async fn load_all(pool: &SqlitePool) -> Vec<Rule> {
    let builtin = load_builtin();
    let user = load_user(pool).await;
    merge(builtin, user)
}

/// Returned by the `list_all_rules` Tauri command — same as Rule but
/// tagged with whether it came from the built-in pack or user DB.
#[derive(Serialize)]
pub struct RuleWithSource {
    #[serde(flatten)]
    pub rule: Rule,
    pub source: &'static str, // "builtin" | "user"
}

/// Like `load_all` but tags each rule with its source. User rules
/// (matched by id) shadow built-in rules, so a built-in id that the
/// user has overridden surfaces as source="user".
pub async fn load_all_with_source(pool: &SqlitePool) -> Vec<RuleWithSource> {
    let builtin_rules = load_builtin();
    let user_rules = load_user(pool).await;
    let user_ids: std::collections::HashSet<String> =
        user_rules.iter().map(|r| r.id.clone()).collect();
    let mut out = Vec::with_capacity(builtin_rules.len() + user_rules.len());
    for r in &builtin_rules {
        if !user_ids.contains(&r.id) {
            out.push(RuleWithSource {
                rule: r.clone(),
                source: "builtin",
            });
        }
    }
    for r in user_rules {
        out.push(RuleWithSource {
            rule: r,
            source: "user",
        });
    }
    out
}

/// Parse all built-in YAML packs. Errors per file are logged but don't
/// abort the rest — one bad pack shouldn't kill the rule engine.
pub fn load_builtin() -> Vec<Rule> {
    let packs = [
        ("credentials", builtin::CREDENTIALS_YAML),
        ("persistence", builtin::PERSISTENCE_YAML),
        ("commands", builtin::COMMANDS_YAML),
        ("network", builtin::NETWORK_YAML),
        ("correlations", builtin::CORRELATIONS_YAML),
        ("metrics", builtin::METRICS_YAML),
    ];
    let mut all = Vec::new();
    for (name, yaml) in packs {
        match serde_yaml::from_str::<RulePack>(yaml) {
            Ok(pack) => {
                tracing::info!(pack = name, n = pack.rules.len(), "loaded builtin rule pack");
                all.extend(pack.rules);
            }
            Err(e) => {
                tracing::error!(pack = name, error = %e, "failed to parse rule pack");
            }
        }
    }
    all
}

/// Load user-defined rules from SQLite. User rules override built-ins with
/// the same id (later in the merged list wins via re-keying).
/// Slice 2: structure in place but no CRUD UI yet — table just stays empty.
pub async fn load_user(pool: &SqlitePool) -> Vec<Rule> {
    let rows: Vec<(String, String, bool)> = match sqlx::query_as(
        "SELECT id, yaml_body, enabled FROM rules",
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "could not load user rules");
            return vec![];
        }
    };
    let mut out = Vec::with_capacity(rows.len());
    for (id, body, enabled) in rows {
        match serde_yaml::from_str::<Rule>(&body) {
            Ok(mut r) => {
                r.id = id;
                r.enabled = enabled;
                out.push(r);
            }
            Err(e) => tracing::warn!(rule_id = id, error = %e, "skipped malformed user rule"),
        }
    }
    out
}

/// Merge built-in + user rules. User rules override built-in ids.
pub fn merge(builtin: Vec<Rule>, user: Vec<Rule>) -> Vec<Rule> {
    let mut by_id: HashMap<String, Rule> = HashMap::new();
    for r in builtin {
        by_id.insert(r.id.clone(), r);
    }
    for r in user {
        by_id.insert(r.id.clone(), r);
    }
    by_id.into_values().collect()
}
