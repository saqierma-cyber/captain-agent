import { useEffect, useMemo, useState } from "react";
import {
  type Finding,
  type FindingStatus,
  type Severity,
  fetchFindings,
  onLiveFinding,
  setFindingStatus,
} from "../api/events";
import { useI18n } from "../i18n";

const MAX_RENDERED = 1000;

const SEVERITY_ORDER: Record<Severity, number> = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
  info: 4,
};

function fmtTime(ns: number): string {
  if (ns === 0) return "-";
  const ms = Math.floor(ns / 1_000_000);
  const d = new Date(ms);
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

const STATUS_OPTIONS: FindingStatus[] = [
  "open",
  "confirmed",
  "dismissed",
  "whitelisted",
];

export function Findings() {
  const { t } = useI18n();
  const [items, setItems] = useState<Finding[]>([]);
  const [filter, setFilter] = useState<FindingStatus | "all">("open");
  const statusLabel = (s: FindingStatus): string =>
    t(`findings.status${s.charAt(0).toUpperCase()}${s.slice(1)}`);

  // Initial fetch + refetch when filter changes.
  useEffect(() => {
    let mounted = true;
    fetchFindings(MAX_RENDERED, filter === "all" ? undefined : filter)
      .then((rows) => {
        if (mounted) setItems(rows);
      })
      .catch((e) => console.error("fetchFindings failed", e));
    return () => {
      mounted = false;
    };
  }, [filter]);

  // Subscribe to live findings stream.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onLiveFinding((f) => {
      setItems((prev) => {
        // Only inject if it matches the current filter
        if (filter !== "all" && f.status !== filter) return prev;
        const next = [f, ...prev];
        return next.length > MAX_RENDERED ? next.slice(0, MAX_RENDERED) : next;
      });
    })
      .then((un) => {
        unlisten = un;
      })
      .catch((e) => console.error("finding subscribe failed", e));
    return () => unlisten?.();
  }, [filter]);

  const sorted = useMemo(
    () =>
      [...items].sort((a, b) => {
        const sd = SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity];
        if (sd !== 0) return sd;
        return b.created_at - a.created_at;
      }),
    [items],
  );

  async function changeStatus(f: Finding, newStatus: FindingStatus) {
    if (f.id == null) return;
    await setFindingStatus(f.id, newStatus);
    setItems((prev) =>
      prev.map((x) => (x.id === f.id ? { ...x, status: newStatus } : x)),
    );
  }

  return (
    <section className="findings">
      <header className="findings-header">
        <h2>{t("findings.title")}</h2>
        <div className="findings-controls">
          <label>{t("findings.statusFilter")}</label>
          <select
            value={filter}
            onChange={(e) => setFilter(e.target.value as FindingStatus | "all")}
          >
            <option value="all">{t("findings.filterAll")}</option>
            {STATUS_OPTIONS.map((s) => (
              <option key={s} value={s}>
                {statusLabel(s)}
              </option>
            ))}
          </select>
          <span className="findings-count">{sorted.length} {t("findings.count")}</span>
        </div>
      </header>
      <ul className="findings-list">
        {sorted.length === 0 ? (
          <li className="findings-empty">
            {t("findings.empty")} <code>cat ~/.ssh/id_*</code>
          </li>
        ) : (
          sorted.map((f) => (
            <li
              key={`finding-${f.id ?? Math.random()}`}
              className={`finding-row sev-${f.severity} status-${f.status}`}
            >
              <span className={`sev-badge sev-${f.severity}`}>{f.severity}</span>
              <span className="ts">{fmtTime(f.created_at)}</span>
              <div className="finding-body">
                <div className="finding-message">{f.message}</div>
                <div className="finding-meta">
                  <span className="rule-id">{f.rule_id}</span>
                  <span className="event-kind">{f.event_kind}</span>
                  <span className="pid">pid={f.pid}</span>
                </div>
                <div className="finding-summary">{f.event_summary}</div>
              </div>
              <select
                value={f.status}
                onChange={(e) =>
                  changeStatus(f, e.target.value as FindingStatus)
                }
                className="status-select"
              >
                {STATUS_OPTIONS.map((s) => (
                  <option key={s} value={s}>
                    {statusLabel(s)}
                  </option>
                ))}
              </select>
            </li>
          ))
        )}
      </ul>
    </section>
  );
}
