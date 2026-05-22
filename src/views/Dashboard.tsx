import { useEffect, useState } from "react";
import {
  type DashboardSummary,
  type Severity,
  fetchDashboard,
} from "../api/events";
import { useI18n } from "../i18n";

const SEVERITY_COLORS: Record<Severity, string> = {
  critical: "#f85149",
  high: "#d29922",
  medium: "#a371f7",
  low: "#4493f8",
  info: "#8b949e",
};

function fmtNum(n: number): string {
  return n.toLocaleString();
}

/** Horizontal severity stacked bar. */
function SeverityBar({ data, emptyLabel }: { data: DashboardSummary["by_severity"]; emptyLabel: string }) {
  const total = data.reduce((s, b) => s + b.count, 0);
  if (total === 0) {
    return <div className="empty">{emptyLabel}</div>;
  }
  return (
    <div className="sev-bar-wrap">
      <div className="sev-bar">
        {data.map((b) => {
          const pct = (b.count / total) * 100;
          return (
            <div
              key={b.severity}
              className="sev-slice"
              style={{
                width: `${pct}%`,
                background: SEVERITY_COLORS[b.severity] ?? "#8b949e",
              }}
              title={`${b.severity}: ${b.count}`}
            >
              {pct >= 6 ? `${b.count}` : ""}
            </div>
          );
        })}
      </div>
      <ul className="sev-legend">
        {data.map((b) => (
          <li key={b.severity}>
            <span
              className="sev-dot"
              style={{ background: SEVERITY_COLORS[b.severity] ?? "#8b949e" }}
            />
            {b.severity} <b>{b.count}</b>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Top rules — horizontal bar list. */
function TopRulesList({ rules, emptyLabel }: { rules: DashboardSummary["top_rules"]; emptyLabel: string }) {
  if (rules.length === 0) {
    return <div className="empty">{emptyLabel}</div>;
  }
  const max = Math.max(...rules.map((r) => r.count));
  return (
    <ul className="top-rules">
      {rules.map((r) => {
        const w = (r.count / max) * 100;
        return (
          <li key={r.rule_id}>
            <span
              className="rule-sev"
              style={{ background: SEVERITY_COLORS[r.severity] ?? "#8b949e" }}
              title={r.severity}
            />
            <span className="rule-id">{r.rule_id}</span>
            <div className="rule-bar-bg">
              <div
                className="rule-bar-fill"
                style={{
                  width: `${w}%`,
                  background: SEVERITY_COLORS[r.severity] ?? "#8b949e",
                }}
              />
            </div>
            <span className="rule-count">{r.count}</span>
          </li>
        );
      })}
    </ul>
  );
}

/** Events-by-hour vertical bar chart over the last 24 hours. */
function HourlyChart({ data, axis24h, axisNow }: { data: DashboardSummary["events_by_hour_24h"]; axis24h: string; axisNow: string }) {
  // Fill in missing hours with 0 so the X-axis is contiguous.
  const now = Math.floor(Date.now() / 1000);
  const startHour = Math.floor(now / 3600) - 23;
  const buckets = new Array(24).fill(0) as number[];
  for (const b of data) {
    const idx = b.hour_start / 3600 - startHour;
    if (idx >= 0 && idx < 24) buckets[idx] = b.count;
  }
  const max = Math.max(1, ...buckets);
  return (
    <div className="hourly-wrap">
      <svg viewBox={`0 0 ${24 * 14} 100`} className="hourly-chart">
        {buckets.map((c, i) => {
          const h = (c / max) * 90;
          return (
            <rect
              key={i}
              x={i * 14 + 2}
              y={100 - h - 2}
              width="10"
              height={h}
              fill="#4493f8"
              opacity={c === 0 ? 0.15 : 0.85}
            >
              <title>
                {new Date((startHour + i) * 3600 * 1000).toLocaleTimeString([], {
                  hour: "2-digit",
                })}
                : {c} events
              </title>
            </rect>
          );
        })}
      </svg>
      <div className="hourly-axis">
        <span>{axis24h}</span>
        <span>{axisNow}</span>
      </div>
    </div>
  );
}

/** Kind breakdown — small chips with counts. */
function KindChips({ data, emptyLabel }: { data: DashboardSummary["events_by_kind_24h"]; emptyLabel: string }) {
  if (data.length === 0) {
    return <div className="empty">{emptyLabel}</div>;
  }
  return (
    <ul className="kind-chips">
      {data.map((k) => (
        <li key={k.kind}>
          <span className="kind-chip">{k.kind}</span>
          <b>{fmtNum(k.count)}</b>
        </li>
      ))}
    </ul>
  );
}

export function Dashboard() {
  const { t } = useI18n();
  const [data, setData] = useState<DashboardSummary | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    const poll = async () => {
      try {
        const d = await fetchDashboard();
        if (mounted) {
          setData(d);
          setErr(null);
        }
      } catch (e) {
        if (mounted) setErr(String(e));
      }
    };
    poll();
    const handle = setInterval(poll, 5000);
    return () => {
      mounted = false;
      clearInterval(handle);
    };
  }, []);

  if (err) {
    return <section className="dashboard"><div className="err">{err}</div></section>;
  }
  if (!data) {
    return <section className="dashboard"><div className="loading">{t("common.loading")}</div></section>;
  }

  return (
    <section className="dashboard">
      <div className="kpi-row">
        <KPI label={t("dashboard.kpiEvents24h")} value={data.events_last_24h} sub={t("dashboard.subInLastHour", { n: data.events_last_hour.toLocaleString() })} />
        <KPI label={t("dashboard.kpiFindings24h")} value={data.findings_last_24h} sub={t("dashboard.subInLastHour", { n: data.findings_last_hour })} />
        <KPI label={t("dashboard.kpiTotalEvents")} value={data.total_events} sub={t("dashboard.subAllTime")} />
        <KPI label={t("dashboard.kpiTotalFindings")} value={data.total_findings} sub={t("dashboard.subAllTime")} />
      </div>

      <div className="dash-row">
        <div className="dash-card">
          <h3>{t("dashboard.bySeverity")}</h3>
          <SeverityBar data={data.by_severity} emptyLabel={t("dashboard.noFindings")} />
        </div>
        <div className="dash-card">
          <h3>{t("dashboard.top10Rules")}</h3>
          <TopRulesList rules={data.top_rules} emptyLabel={t("dashboard.noRules")} />
        </div>
      </div>

      <div className="dash-row">
        <div className="dash-card dash-card-wide">
          <h3>{t("dashboard.eventsPerHour")}</h3>
          <HourlyChart data={data.events_by_hour_24h} axis24h={t("dashboard.hourAgo24")} axisNow={t("dashboard.now")} />
        </div>
      </div>

      <div className="dash-row">
        <div className="dash-card dash-card-wide">
          <h3>{t("dashboard.eventsByKind")}</h3>
          <KindChips data={data.events_by_kind_24h} emptyLabel={t("dashboard.noEvents24h")} />
        </div>
      </div>
    </section>
  );
}

function KPI({
  label,
  value,
  sub,
}: {
  label: string;
  value: number;
  sub: string;
}) {
  return (
    <div className="kpi">
      <div className="kpi-label">{label}</div>
      <div className="kpi-value">{fmtNum(value)}</div>
      <div className="kpi-sub">{sub}</div>
    </div>
  );
}
