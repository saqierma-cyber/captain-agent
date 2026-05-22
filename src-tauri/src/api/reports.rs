//! HTML report generator — writes a self-contained file (inline CSS,
//! no JS, no external assets) with findings in a chosen time range.

use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};

pub async fn generate_html_report(
    pool: &SqlitePool,
    out_dir: &Path,
    start_ts_ns: i64,
    end_ts_ns: i64,
    severity_filter: Option<&str>,
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(out_dir).await.ok();

    // Findings rows
    let where_sev = if severity_filter.is_some() {
        " AND severity = ?3"
    } else {
        ""
    };
    let sql = format!(
        "SELECT id, datetime(created_at/1000000000, 'unixepoch', 'localtime') AS t, \
                severity, rule_id, message, status, pid, event_kind, event_summary \
         FROM findings \
         WHERE created_at >= ?1 AND created_at < ?2 {where_sev} \
         ORDER BY \
           CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 \
                         WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END, \
           created_at DESC"
    );
    let mut q = sqlx::query(&sql).bind(start_ts_ns).bind(end_ts_ns);
    if let Some(s) = severity_filter {
        q = q.bind(s);
    }
    let rows = q.fetch_all(pool).await.context("query findings")?;

    // Severity counts
    let totals: Vec<(String, i64)> = sqlx::query_as(
        "SELECT severity, COUNT(*) FROM findings \
         WHERE created_at >= ?1 AND created_at < ?2 GROUP BY severity",
    )
    .bind(start_ts_ns)
    .bind(end_ts_ns)
    .fetch_all(pool)
    .await
    .context("severity totals")?;

    let total = rows.len();

    // Assemble HTML.
    let mut html = String::new();
    html.push_str(HTML_HEAD);

    let started = format_ts(start_ts_ns);
    let ended = format_ts(end_ts_ns);
    let now = format_ts(
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_else(|| chrono::Utc::now().timestamp() * 1_000_000_000),
    );
    html.push_str(&format!(
        r#"<h1>Captain Agent — Findings Report</h1>
<p class="meta">Range: <code>{started}</code> → <code>{ended}</code><br>
Generated: <code>{now}</code><br>
Total findings: <b>{total}</b></p>"#
    ));

    if !totals.is_empty() {
        html.push_str(r#"<h2>By severity</h2><ul class="sev-list">"#);
        for (sev, n) in &totals {
            html.push_str(&format!(
                r#"<li><span class="sev sev-{sev}">{sev}</span> {n}</li>"#
            ));
        }
        html.push_str("</ul>");
    }

    html.push_str("<h2>Findings</h2>");
    if rows.is_empty() {
        html.push_str("<p><i>No findings in this range.</i></p>");
    } else {
        html.push_str(
            r#"<table><thead><tr>
<th>Severity</th><th>Time</th><th>Rule</th><th>PID</th><th>Event</th><th>Status</th>
</tr></thead><tbody>"#,
        );
        for row in &rows {
            let sev: String = row.get("severity");
            let t: String = row.get("t");
            let rule_id: String = row.get("rule_id");
            let pid: i64 = row.get("pid");
            let event_kind: String = row.get("event_kind");
            let event_summary: String = row.get("event_summary");
            let message: String = row.get("message");
            let status: String = row.get("status");
            html.push_str(&format!(
                r#"<tr><td><span class="sev sev-{sev}">{sev}</span></td>
<td><code>{t}</code></td>
<td><code>{rule_id}</code><br><span class="msg">{}</span></td>
<td><code>{pid}</code></td>
<td><code class="kind">{event_kind}</code> {}</td>
<td><span class="status status-{status}">{status}</span></td></tr>"#,
                html_escape(&message),
                html_escape(&event_summary),
            ));
        }
        html.push_str("</tbody></table>");
    }
    html.push_str("</body></html>");

    let filename = format!(
        "captain-report-{}.html",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = out_dir.join(filename);
    tokio::fs::write(&path, html)
        .await
        .with_context(|| format!("write report to {}", path.display()))?;
    Ok(path)
}

fn format_ts(ns: i64) -> String {
    let secs = ns / 1_000_000_000;
    let datetime = chrono::DateTime::<chrono::Local>::from(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
    );
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const HTML_HEAD: &str = r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8">
<title>Captain Agent — Findings Report</title>
<style>
  :root { color-scheme: light dark; }
  body { font-family: -apple-system, "Inter", system-ui, sans-serif; margin: 24px; max-width: 1200px; }
  h1 { font-size: 22px; margin-bottom: 4px; }
  h2 { font-size: 16px; margin-top: 24px; border-bottom: 1px solid #ccc; padding-bottom: 4px; }
  .meta { color: #666; font-size: 13px; }
  code { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 12px; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th, td { padding: 8px 6px; text-align: left; border-bottom: 1px solid #e5e7eb; vertical-align: top; }
  th { font-weight: 600; background: #f9fafb; }
  .sev { display: inline-block; padding: 2px 8px; border-radius: 3px; font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; }
  .sev-critical { background: #f85149; color: #fff; }
  .sev-high { background: #d29922; color: #000; }
  .sev-medium { background: #a371f7; color: #fff; }
  .sev-low { background: #4493f8; color: #fff; }
  .sev-info { background: #8b949e; color: #fff; }
  .sev-list { list-style: none; padding: 0; display: flex; gap: 16px; }
  .msg { color: #444; }
  .kind { background: #f3f4f6; padding: 1px 5px; border-radius: 3px; }
  .status { font-size: 11px; padding: 1px 5px; border-radius: 3px; background: #e5e7eb; }
  .status-open { background: #fef3c7; color: #92400e; }
  .status-confirmed { background: #fee2e2; color: #991b1b; }
  .status-dismissed { background: #f3f4f6; color: #6b7280; }
  .status-whitelisted { background: #dbeafe; color: #1e40af; }
  @media (prefers-color-scheme: dark) {
    body { background: #0e1117; color: #e6edf3; }
    .meta { color: #8b949e; }
    th { background: #161b22; }
    th, td { border-color: #30363d; }
    .msg { color: #cdd9e5; }
    .kind { background: #21262d; color: #cdd9e5; }
    .status { background: #21262d; color: #cdd9e5; }
  }
</style></head><body>
"#;
