import { useEffect, useRef, useState } from "react";
import {
  type LiveEvent,
  type EventRow,
  fetchRecentEvents,
  onLiveEvent,
  summarize,
  eventKind,
} from "../api/events";
import { useI18n } from "../i18n";

/** Cap the in-memory live event list so the UI doesn't grow unbounded. */
const MAX_RENDERED = 1000;

type Row = {
  key: string;
  kind: string;
  pid: number;
  parent_pid: number | null;
  ts: number;
  summary: string;
};

function rowFromLive(ev: LiveEvent): Row {
  return {
    key: `live-${ev.ts}-${ev.pid}-${eventKind(ev)}-${Math.random().toString(36).slice(2)}`,
    kind: eventKind(ev),
    pid: ev.pid,
    parent_pid: ev.parent_pid,
    ts: ev.ts,
    summary: summarize(ev),
  };
}

function rowFromStored(r: EventRow): Row {
  // detail_json is a serialized EventDetail enum; parse it.
  let summary = r.detail_json.slice(0, 120);
  try {
    const parsed = JSON.parse(r.detail_json);
    if (parsed && typeof parsed === "object") {
      // EventDetail is serialized with `kind` + `detail` so we can route.
      const tagged: LiveEvent = {
        id: r.id,
        session_id: null,
        pid: r.pid,
        parent_pid: r.parent_pid,
        ts: r.ts,
        detail: parsed,
      };
      summary = summarize(tagged);
    }
  } catch {
    /* keep fallback */
  }
  return {
    key: `stored-${r.id}`,
    kind: r.kind,
    pid: r.pid,
    parent_pid: r.parent_pid,
    ts: r.ts,
    summary,
  };
}

function fmtTime(ns: number): string {
  if (ns === 0) return "-";
  const ms = Math.floor(ns / 1_000_000);
  const d = new Date(ms);
  // HH:MM:SS.mmm
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

export function Timeline() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Row[]>([]);
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Initial load: pull recent events from DB.
  useEffect(() => {
    let mounted = true;
    fetchRecentEvents(200)
      .then((records) => {
        if (!mounted) return;
        // Records come back DESC by ts; reverse so newest is at top after append.
        setRows(records.map(rowFromStored));
      })
      .catch((e) => console.error("fetchRecentEvents failed", e));
    return () => {
      mounted = false;
    };
  }, []);

  // Subscribe to live events.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onLiveEvent((ev) => {
      if (pausedRef.current) return;
      setRows((prev) => {
        const next = [rowFromLive(ev), ...prev];
        return next.length > MAX_RENDERED ? next.slice(0, MAX_RENDERED) : next;
      });
    })
      .then((un) => {
        unlisten = un;
      })
      .catch((e) => console.error("event subscribe failed", e));
    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <section className="timeline">
      <header className="timeline-header">
        <h2>{t("timeline.title")}</h2>
        <div className="timeline-controls">
          <span className="timeline-count">{rows.length} {t("timeline.count")}</span>
          <button onClick={() => setPaused((p) => !p)}>
            {paused ? t("timeline.resume") : t("timeline.pause")}
          </button>
          <button onClick={() => setRows([])}>{t("timeline.clear")}</button>
        </div>
      </header>
      <ul className="timeline-list">
        {rows.length === 0 ? (
          <li className="timeline-empty">{t("timeline.empty")}</li>
        ) : (
          rows.map((r) => (
            <li key={r.key} className={`timeline-row kind-${r.kind}`}>
              <span className="ts">{fmtTime(r.ts)}</span>
              <span className="kind">{r.kind}</span>
              <span className="pid">
                pid={r.pid}
                {r.parent_pid !== null && r.parent_pid !== undefined ? (
                  <span className="ppid"> ← {r.parent_pid}</span>
                ) : null}
              </span>
              <span className="summary">{r.summary}</span>
            </li>
          ))
        )}
      </ul>
    </section>
  );
}
