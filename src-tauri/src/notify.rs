//! ⑧ OS-level notifications for high-severity findings.
//!
//! Slice 4: pop a macOS Notification Center toast for `critical` and
//! `high` findings. We fire-and-forget on a blocking task — notify-rust
//! synchronously talks to NSUserNotificationCenter, and we don't want
//! the rule engine to block on it.

use captain_common::{Finding, Severity};

pub fn maybe_notify(f: &Finding) {
    if !should_notify(f.severity) {
        return;
    }
    let title = format!(
        "Captain Agent — {} {}",
        severity_label(f.severity),
        f.rule_id
    );
    let body = if f.event_summary.is_empty() {
        f.message.clone()
    } else {
        format!("{}\n\n{}", f.message, truncate(&f.event_summary, 200))
    };
    tokio::task::spawn_blocking(move || {
        let result = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .timeout(notify_rust::Timeout::Milliseconds(15_000))
            .show();
        if let Err(e) = result {
            tracing::warn!(error = %e, "failed to send OS notification");
        }
    });
}

fn should_notify(sev: Severity) -> bool {
    matches!(sev, Severity::Critical | Severity::High)
}

fn severity_label(sev: Severity) -> &'static str {
    match sev {
        Severity::Critical => "Critical",
        Severity::High => "High",
        Severity::Medium => "Medium",
        Severity::Low => "Low",
        Severity::Info => "Info",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    // UTF-8 safe: take up to `max - 1` chars by char-boundary, not byte index.
    // event_summary often contains Chinese paths — naive &s[..n] panics if
    // n lands inside a multi-byte char.
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}
