import { useEffect, useMemo, useState } from "react";
import {
  type RuleSource,
  type RuleType,
  type RuleWithSource,
  type Severity,
  listAllRules,
  upsertUserRule,
  deleteUserRule,
  setRuleEnabled,
} from "../api/events";
import { useI18n } from "../i18n";

const SEVERITY_COLORS: Record<Severity, string> = {
  critical: "#f85149",
  high: "#d29922",
  medium: "#a371f7",
  low: "#4493f8",
  info: "#8b949e",
};

const SAMPLE_YAML = `id: my-custom-rule
type: process
severity: medium
cmd_pattern: 'docker\\s+(run|exec).*--privileged'
message: "docker run/exec with --privileged"
enabled: true
`;

export function Rules() {
  const { t } = useI18n();
  const [rules, setRules] = useState<RuleWithSource[]>([]);
  const [filterType, setFilterType] = useState<"all" | RuleType>("all");
  const [filterSource, setFilterSource] = useState<"all" | RuleSource>("all");
  const [showAdd, setShowAdd] = useState(false);
  const [yamlBody, setYamlBody] = useState(SAMPLE_YAML);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      setRules(await listAllRules());
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  };
  useEffect(() => {
    refresh();
  }, []);

  const filtered = useMemo(
    () =>
      rules.filter((r) => {
        if (filterType !== "all" && r.type !== filterType) return false;
        if (filterSource !== "all" && r.source !== filterSource) return false;
        return true;
      }),
    [rules, filterType, filterSource],
  );

  async function handleToggle(r: RuleWithSource) {
    setBusy(true);
    try {
      await setRuleEnabled(r.id, !r.enabled);
      await refresh();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(r: RuleWithSource) {
    if (r.source === "builtin") {
      // For built-ins, "delete" means "remove the user override row".
      // We can't actually delete built-in rules from the YAML pack.
      setErr(t("rules.cannotDeleteBuiltin"));
      return;
    }
    if (!confirm(t("rules.confirmDelete", { id: r.id }))) return;
    setBusy(true);
    try {
      await deleteUserRule(r.id);
      await refresh();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      await upsertUserRule(yamlBody);
      setShowAdd(false);
      setYamlBody(SAMPLE_YAML);
      await refresh();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  const countsBySource = useMemo(() => {
    const byBuiltin = rules.filter((r) => r.source === "builtin").length;
    const byUser = rules.filter((r) => r.source === "user").length;
    return { byBuiltin, byUser };
  }, [rules]);

  return (
    <section className="rules">
      <header className="rules-header">
        <h2>{t("rules.title")}</h2>
        <div className="rules-meta">
          <span>{t("rules.countBuiltin", { n: countsBySource.byBuiltin })}</span>
          <span>{t("rules.countUser", { n: countsBySource.byUser })}</span>
          <button onClick={() => setShowAdd(true)} disabled={busy}>
            + {t("rules.addBtn")}
          </button>
        </div>
      </header>

      <div className="rules-filter-bar">
        <label>{t("rules.filterType")}</label>
        <select
          value={filterType}
          onChange={(e) => setFilterType(e.target.value as any)}
        >
          <option value="all">{t("rules.filterAll")}</option>
          <option value="file">file</option>
          <option value="process">process</option>
          <option value="network">network</option>
          <option value="correlation">correlation</option>
          <option value="metric">metric</option>
        </select>
        <label>{t("rules.filterSource")}</label>
        <select
          value={filterSource}
          onChange={(e) => setFilterSource(e.target.value as any)}
        >
          <option value="all">{t("rules.filterAll")}</option>
          <option value="builtin">builtin</option>
          <option value="user">user</option>
        </select>
      </div>

      {err ? <div className="rules-error">{err}</div> : null}

      <ul className="rules-list">
        {filtered.length === 0 ? (
          <li className="rules-empty">{t("rules.empty")}</li>
        ) : (
          filtered.map((r) => (
            <li
              key={r.id}
              className={`rule-row ${r.enabled ? "" : "disabled"} source-${r.source}`}
            >
              <span
                className="rule-sev-dot"
                style={{ background: SEVERITY_COLORS[r.severity] ?? "#8b949e" }}
                title={r.severity}
              />
              <div className="rule-info">
                <div className="rule-row-title">
                  <span className="rule-id">{r.id}</span>
                  <span className={`source-badge source-${r.source}`}>
                    {r.source}
                  </span>
                  <span className="rule-type-badge">{r.type}</span>
                  <span className={`sev-tag sev-${r.severity}`}>
                    {r.severity}
                  </span>
                </div>
                {r.message ? <div className="rule-msg">{r.message}</div> : null}
                <div className="rule-detail">
                  {r.pattern ? (
                    <code>pattern: {r.pattern}</code>
                  ) : null}
                  {r.cmd_pattern ? (
                    <code>cmd_pattern: {r.cmd_pattern}</code>
                  ) : null}
                  {r.domains && r.domains.length ? (
                    <code>domains: {r.domains.slice(0, 3).join(", ")}{r.domains.length > 3 ? "…" : ""}</code>
                  ) : null}
                  {r.metric ? (
                    <code>metric: {r.metric} ≥ {r.threshold} / {r.window_seconds}s</code>
                  ) : null}
                  {r.require_all_rules && r.require_all_rules.length ? (
                    <code>require: {r.require_all_rules.join(" + ")} ({r.window_seconds}s)</code>
                  ) : null}
                </div>
              </div>
              <div className="rule-actions">
                <button
                  className={`toggle ${r.enabled ? "on" : "off"}`}
                  onClick={() => handleToggle(r)}
                  disabled={busy}
                >
                  {r.enabled ? "ON" : "OFF"}
                </button>
                <button
                  className="remove"
                  onClick={() => handleDelete(r)}
                  disabled={busy || r.source === "builtin"}
                  title={r.source === "builtin" ? t("rules.cannotDeleteBuiltin") : ""}
                >
                  ✕
                </button>
              </div>
            </li>
          ))
        )}
      </ul>

      {showAdd ? (
        <div className="rules-modal-backdrop" onClick={() => setShowAdd(false)}>
          <div className="rules-modal" onClick={(e) => e.stopPropagation()}>
            <h3>{t("rules.addModalTitle")}</h3>
            <p className="hint">{t("rules.addModalHint")}</p>
            <form onSubmit={handleSubmit}>
              <textarea
                value={yamlBody}
                onChange={(e) => setYamlBody(e.target.value)}
                rows={14}
                spellCheck={false}
              />
              <div className="rules-modal-actions">
                <button type="button" onClick={() => setShowAdd(false)}>
                  {t("common.cancel")}
                </button>
                <button type="submit" disabled={busy}>
                  {t("rules.saveBtn")}
                </button>
              </div>
            </form>
          </div>
        </div>
      ) : null}
    </section>
  );
}
