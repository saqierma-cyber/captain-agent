//! Single-event matchers — match one event against one rule, return Finding.

use captain_common::{Event, EventDetail, Finding, FindingStatus, Rule, RuleType};

pub fn evaluate(rule: &Rule, ev: &Event) -> Option<Finding> {
    if !rule.enabled {
        return None;
    }
    match (rule.rule_type, &ev.detail) {
        (RuleType::File, EventDetail::FileRead { path }) => {
            match_file(rule, ev, path, "read")
        }
        (RuleType::File, EventDetail::FileWrite { path }) => {
            match_file(rule, ev, path, "write")
        }
        (RuleType::File, EventDetail::FileDelete { path }) => {
            match_file(rule, ev, path, "delete")
        }
        (RuleType::Process, EventDetail::ProcessSpawn { cmdline, exe, .. }) => {
            // Match against either cmdline or exe path — covers both
            // "curl -X POST" patterns and bare path matches.
            match_process(rule, ev, cmdline, exe)
        }
        (RuleType::Network, EventDetail::NetConnect { remote_addr, .. }) => {
            match_network(rule, ev, remote_addr)
        }
        (RuleType::Network, EventDetail::DnsQuery { domain }) => {
            match_network(rule, ev, domain)
        }
        _ => None,
    }
}

fn match_file(rule: &Rule, ev: &Event, path: &str, action: &str) -> Option<Finding> {
    if let Some(req_action) = &rule.action {
        if !req_action.eq_ignore_ascii_case(action) {
            return None;
        }
    }
    let pattern = rule.pattern.as_ref()?;
    let expanded = expand_pattern(pattern);
    let pat = glob::Pattern::new(&expanded).ok()?;
    if !pat.matches(path) {
        return None;
    }
    Some(build_finding(rule, ev, path))
}

fn match_process(rule: &Rule, ev: &Event, cmdline: &str, exe: &str) -> Option<Finding> {
    let pattern = rule.cmd_pattern.as_ref()?;
    let re = regex::Regex::new(pattern).ok()?;
    if !re.is_match(cmdline) && !re.is_match(exe) {
        return None;
    }
    let summary = if !cmdline.is_empty() { cmdline } else { exe };
    Some(build_finding(rule, ev, summary))
}

fn match_network(rule: &Rule, ev: &Event, target: &str) -> Option<Finding> {
    let list = rule.domains.as_ref()?;
    if !list.iter().any(|d| target.contains(d)) {
        return None;
    }
    Some(build_finding(rule, ev, target))
}

fn build_finding(rule: &Rule, ev: &Event, summary: &str) -> Finding {
    let message = rule
        .message
        .clone()
        .unwrap_or_else(|| format!("matched rule {}", rule.id));
    let summary_short = if summary.len() > 240 {
        format!("{}…", &summary[..237])
    } else {
        summary.to_string()
    };
    Finding {
        id: None,
        event_id: None,
        rule_id: rule.id.clone(),
        severity: rule.severity,
        message,
        status: FindingStatus::Open,
        notes: None,
        created_at: chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000),
        pid: ev.pid,
        event_kind: ev.detail.kind_str().to_string(),
        event_summary: summary_short,
    }
}

/// Expand `~/foo/bar` to `/Users/*/foo/bar` so cross-user patterns just work.
fn expand_pattern(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        format!("/Users/*/{rest}")
    } else if p == "~" {
        "/Users/*".to_string()
    } else {
        p.to_string()
    }
}
