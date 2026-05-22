import { useEffect, useState } from "react";
import {
  type Target,
  type TargetMatchKind,
  type TargetStatus,
  listTargets,
  addTarget,
  removeTarget,
  toggleTarget,
} from "../api/events";
import { useI18n } from "../i18n";

function fmtTime(ns: number): string {
  if (ns === 0) return "-";
  return new Date(Math.floor(ns / 1_000_000)).toLocaleString();
}

export function Targets() {
  const { t } = useI18n();
  const KIND_OPTIONS: { value: TargetMatchKind; label: string; hint: string }[] = [
    { value: "exe_path", label: t("targets.kind.exePath"), hint: t("targets.kindHint.exePath") },
    { value: "exe_prefix", label: t("targets.kind.exePrefix"), hint: t("targets.kindHint.exePrefix") },
    { value: "process_name", label: t("targets.kind.processName"), hint: t("targets.kindHint.processName") },
    { value: "bundle_id", label: t("targets.kind.bundleId"), hint: t("targets.kindHint.bundleId") },
  ];

  const [status, setStatus] = useState<TargetStatus | null>(null);
  const [name, setName] = useState("");
  const [kind, setKind] = useState<TargetMatchKind>("exe_prefix");
  const [value, setValue] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      setStatus(await listTargets());
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  };

  useEffect(() => {
    refresh();
    const handle = setInterval(refresh, 3000);
    return () => clearInterval(handle);
  }, []);

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim() || !value.trim()) {
      setErr(t("targets.errNameValueRequired"));
      return;
    }
    setBusy(true);
    try {
      await addTarget(name.trim(), kind, value.trim());
      setName("");
      setValue("");
      await refresh();
    } catch (ex) {
      setErr(String(ex));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggle(t: Target) {
    if (t.id == null) return;
    await toggleTarget(t.id, !t.enabled);
    await refresh();
  }

  async function handleRemove(tgt: Target) {
    if (tgt.id == null) return;
    if (!confirm(t("targets.confirmRemove", { name: tgt.name }))) return;
    await removeTarget(tgt.id);
    await refresh();
  }

  return (
    <section className="targets">
      <header className="targets-header">
        <h2>{t("targets.title")}</h2>
        <div className="targets-meta">
          {status ? (
            <>
              <span className={status.mode === "filter" ? "mode-filter" : "mode-all"}>
                {status.mode === "filter" ? t("targets.modeFilter") : t("targets.modeShowAll")}
              </span>
              <span className="pid-count">
                {t("targets.livePids", { n: status.monitored_pid_count })}
              </span>
            </>
          ) : (
            <span>{t("common.loading")}</span>
          )}
        </div>
      </header>

      <form className="target-add" onSubmit={handleAdd}>
        <input
          placeholder={t("targets.namePlaceholder")}
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
        />
        <select
          value={kind}
          onChange={(e) => setKind(e.target.value as TargetMatchKind)}
        >
          {KIND_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
        <input
          placeholder={KIND_OPTIONS.find((o) => o.value === kind)?.hint ?? ""}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          required
        />
        <button type="submit" disabled={busy}>
          {busy ? t("targets.adding") : t("targets.addBtn")}
        </button>
      </form>
      {err ? <div className="targets-error">{err}</div> : null}

      <ul className="targets-list">
        {status?.targets.length === 0 ? (
          <li className="targets-empty">{t("targets.empty")}</li>
        ) : (
          status?.targets.map((tgt) => (
            <li
              key={`target-${tgt.id}`}
              className={`target-row ${tgt.enabled ? "" : "disabled"}`}
            >
              <button
                className={`toggle ${tgt.enabled ? "on" : "off"}`}
                onClick={() => handleToggle(tgt)}
                title={tgt.enabled ? t("findings.statusDismissed") : t("findings.statusOpen")}
              >
                {tgt.enabled ? "ON" : "OFF"}
              </button>
              <div className="target-body">
                <div className="target-name">{tgt.name}</div>
                <div className="target-meta">
                  <span className="kind">{tgt.match_kind}</span>
                  <span className="value">{tgt.match_value}</span>
                  <span className="ts">{t("targets.addedAt", { date: fmtTime(tgt.created_at) })}</span>
                </div>
              </div>
              <button className="remove" onClick={() => handleRemove(tgt)}>
                ✕
              </button>
            </li>
          ))
        )}
      </ul>
    </section>
  );
}
