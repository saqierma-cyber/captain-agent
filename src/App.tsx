import { useEffect, useState } from "react";
import { Dashboard } from "./views/Dashboard";
import { Timeline } from "./views/Timeline";
import { Findings } from "./views/Findings";
import { Targets } from "./views/Targets";
import { Rules } from "./views/Rules";
import { Settings } from "./views/Settings";
import { fetchStatus, type Status } from "./api/events";
import { useI18n } from "./i18n";
import "./App.css";

type Tab = "dashboard" | "findings" | "timeline" | "targets" | "rules" | "settings";

function App() {
  const { t } = useI18n();
  const [status, setStatus] = useState<Status | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("dashboard");

  useEffect(() => {
    let mounted = true;
    const poll = async () => {
      try {
        const s = await fetchStatus();
        if (mounted) {
          setStatus(s);
          setStatusError(null);
        }
      } catch (e) {
        if (mounted) setStatusError(String(e));
      }
    };
    poll();
    const handle = setInterval(poll, 3000);
    return () => {
      mounted = false;
      clearInterval(handle);
    };
  }, []);

  return (
    <div className="app">
      <nav className="app-nav">
        <h1 className="app-brand">{t("app.brand")}</h1>
        <div className="app-tabs">
          <button
            className={tab === "dashboard" ? "tab-active" : ""}
            onClick={() => setTab("dashboard")}
          >
            {t("nav.dashboard")}
          </button>
          <button
            className={tab === "findings" ? "tab-active" : ""}
            onClick={() => setTab("findings")}
          >
            {t("nav.findings")}
            {status && status.findings_open > 0 ? (
              <span className="tab-badge">{status.findings_open}</span>
            ) : null}
          </button>
          <button
            className={tab === "timeline" ? "tab-active" : ""}
            onClick={() => setTab("timeline")}
          >
            {t("nav.timeline")}
          </button>
          <button
            className={tab === "targets" ? "tab-active" : ""}
            onClick={() => setTab("targets")}
          >
            {t("nav.targets")}
            {status && status.target_count > 0 ? (
              <span className="tab-badge tab-badge-neutral">
                {status.target_count}
              </span>
            ) : null}
          </button>
          <button
            className={tab === "rules" ? "tab-active" : ""}
            onClick={() => setTab("rules")}
          >
            {t("nav.rules")}
          </button>
          <button
            className={tab === "settings" ? "tab-active" : ""}
            onClick={() => setTab("settings")}
          >
            {t("nav.settings")}
          </button>
        </div>
        <div className="app-status">
          {statusError ? (
            <span className="status-bad">{t("status.backendUnreachable")}</span>
          ) : status ? (
            <>
              <span
                className={status.helper_connected ? "status-ok" : "status-bad"}
              >
                {status.helper_connected
                  ? t("status.connected")
                  : t("status.offline")}
              </span>
              <span className="status-count">
                {status.mode === "filter"
                  ? `${status.monitored_pid_count}${t("status.pidsSuffix")}`
                  : ""}
                {status.events_total.toLocaleString()} {t("status.eventsLabel")} ·{" "}
                {status.findings_total.toLocaleString()} {t("status.findingsLabel")}
              </span>
            </>
          ) : (
            <span>{t("status.loading")}</span>
          )}
        </div>
      </nav>
      <main className="app-main">
        {tab === "dashboard" ? (
          <Dashboard />
        ) : tab === "findings" ? (
          <Findings />
        ) : tab === "timeline" ? (
          <Timeline />
        ) : tab === "targets" ? (
          <Targets />
        ) : tab === "rules" ? (
          <Rules />
        ) : (
          <Settings />
        )}
      </main>
    </div>
  );
}

export default App;
