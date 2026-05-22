import { useEffect, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  type DataStats,
  type Severity,
  clearAllData,
  clearOldEvents,
  exportReport,
  getDataStats,
} from "../api/events";
import { useI18n, LOCALES, type Locale } from "../i18n";

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function Settings() {
  const { t, locale, setLocale } = useI18n();
  const [stats, setStats] = useState<DataStats | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [days, setDays] = useState(30);
  const [rangeHours, setRangeHours] = useState(24);
  const [severity, setSeverity] = useState<Severity | "all">("all");
  const [lastPath, setLastPath] = useState<string | null>(null);

  const RANGE_OPTIONS = [
    { hours: 1, label: t("settings.export.range1h") },
    { hours: 24, label: t("settings.export.range24h") },
    { hours: 24 * 7, label: t("settings.export.range7d") },
    { hours: 24 * 30, label: t("settings.export.range30d") },
    { hours: -1, label: t("settings.export.rangeAll") },
  ];

  const SEVERITY_OPTIONS: { value: Severity | "all"; label: string }[] = [
    { value: "all", label: t("settings.export.allSev") },
    { value: "critical", label: t("settings.export.critOnly") },
    { value: "high", label: t("settings.export.highOnly") },
    { value: "medium", label: t("settings.export.medOnly") },
    { value: "low", label: t("settings.export.lowOnly") },
  ];

  const refresh = async () => {
    try {
      setStats(await getDataStats());
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  async function handleClearOld() {
    if (!confirm(t("settings.retention.confirmDelete", { days }))) return;
    setBusy(true);
    try {
      const deleted = await clearOldEvents(days);
      setMsg(t("settings.retention.deleted", { n: deleted.toLocaleString() }));
      await refresh();
    } catch (e) {
      setMsg(`Error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleClearAll() {
    if (!confirm(t("settings.retention.confirmWipe"))) return;
    if (!confirm(t("settings.retention.reallyWipe"))) return;
    setBusy(true);
    try {
      await clearAllData();
      setMsg(t("settings.retention.wiped"));
      await refresh();
    } catch (e) {
      setMsg(`Error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleExport() {
    setBusy(true);
    setMsg(null);
    try {
      const now = Date.now() * 1_000_000;
      const start =
        rangeHours < 0 ? 0 : now - rangeHours * 3600 * 1_000_000_000;
      const sev = severity === "all" ? undefined : severity;
      const path = await exportReport(start, now, sev);
      setLastPath(path);
      setMsg(t("settings.export.savedAt", { path }));
      try {
        await openPath(path);
      } catch (e) {
        setMsg(
          t("settings.export.autoOpenFailed", { path, err: String(e) }),
        );
      }
    } catch (e) {
      setMsg(`Error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function openLast() {
    if (!lastPath) return;
    try {
      await openPath(lastPath);
    } catch (e) {
      setMsg(`Open failed: ${e}`);
    }
  }

  return (
    <section className="settings">
      {/* Language */}
      <div className="settings-section">
        <h3>{t("settings.langSection")}</h3>
        <div className="settings-row">
          <label>{t("settings.langCurrent")}</label>
          <select
            value={locale}
            onChange={(e) => setLocale(e.target.value as Locale)}
          >
            {LOCALES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.nativeLabel} ({l.label})
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Storage */}
      <div className="settings-section">
        <h3>{t("settings.storage")}</h3>
        {stats ? (
          <table className="settings-stats">
            <tbody>
              <tr>
                <td>{t("settings.storage.dbSize")}</td>
                <td><b>{fmtBytes(stats.db_bytes)}</b></td>
              </tr>
              <tr>
                <td>{t("settings.storage.eventsTotal")}</td>
                <td><b>{stats.events_total.toLocaleString()}</b></td>
              </tr>
              <tr>
                <td>{t("settings.storage.findingsTotal")}</td>
                <td><b>{stats.findings_total.toLocaleString()}</b></td>
              </tr>
              <tr>
                <td>{t("settings.storage.oldestEvent")}</td>
                <td>
                  <code>
                    {stats.oldest_event_ts
                      ? new Date(stats.oldest_event_ts / 1_000_000).toLocaleString()
                      : "—"}
                  </code>
                </td>
              </tr>
            </tbody>
          </table>
        ) : (
          <span>{t("common.loading")}</span>
        )}
      </div>

      {/* Retention */}
      <div className="settings-section">
        <h3>{t("settings.retention")}</h3>
        <div className="settings-row">
          <label>{t("settings.retention.deleteOlderThan")}</label>
          <input
            type="number"
            min={1}
            value={days}
            onChange={(e) => setDays(parseInt(e.target.value) || 30)}
            style={{ width: 70 }}
          />
          <span>{t("settings.retention.days")}</span>
          <button onClick={handleClearOld} disabled={busy}>
            {t("settings.retention.deleteBtn")}
          </button>
        </div>
        <div className="settings-row">
          <button onClick={handleClearAll} disabled={busy} className="btn-danger">
            {t("settings.retention.wipeBtn")}
          </button>
          <span className="hint">{t("settings.retention.wipeHint")}</span>
        </div>
      </div>

      {/* Export */}
      <div className="settings-section">
        <h3>{t("settings.export")}</h3>
        <div className="settings-row">
          <label>{t("settings.export.range")}</label>
          <select
            value={rangeHours}
            onChange={(e) => setRangeHours(parseInt(e.target.value))}
          >
            {RANGE_OPTIONS.map((o) => (
              <option key={o.label} value={o.hours}>
                {o.label}
              </option>
            ))}
          </select>
          <label>{t("settings.export.severity")}</label>
          <select
            value={severity}
            onChange={(e) => setSeverity(e.target.value as Severity | "all")}
          >
            {SEVERITY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <button onClick={handleExport} disabled={busy}>
            {t("settings.export.generate")}
          </button>
          {lastPath ? (
            <button onClick={openLast} disabled={busy}>
              {t("settings.export.openLast")}
            </button>
          ) : null}
        </div>
      </div>

      {/* Diagnostics */}
      <div className="settings-section">
        <h3>{t("settings.diagnostics")}</h3>
        <p className="hint">{t("settings.diagnostics.hint")}</p>
        <pre className="settings-code">
sudo ./scripts/captain-diagnose.sh
        </pre>
        <p className="hint">
          {/* Path is relative to the Captain Agent source checkout. For
              packaged installs, the script lives at
              /usr/local/libexec/captain-diagnose.sh (V2 — Slice 5 packaging). */}
        </p>
      </div>

      {msg ? <div className="settings-msg">{msg}</div> : null}
    </section>
  );
}
