// TypeScript bindings for the captain-agent backend API.
// Keep in sync with captain-common (event.rs, finding.rs, rule.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type EventKind =
  | "process_spawn"
  | "process_exit"
  | "file_read"
  | "file_write"
  | "file_delete"
  | "net_connect"
  | "dns_query"
  | "persistence";

export type Severity = "info" | "low" | "medium" | "high" | "critical";

export type FindingStatus =
  | "open"
  | "confirmed"
  | "dismissed"
  | "whitelisted";

export type EventDetail =
  | { kind: "process_spawn"; detail: { exe: string; cmdline: string; uid: number | null } }
  | { kind: "process_exit"; detail: { exit_code: number | null } }
  | { kind: "file_read"; detail: { path: string } }
  | { kind: "file_write"; detail: { path: string } }
  | { kind: "file_delete"; detail: { path: string } }
  | { kind: "net_connect"; detail: { remote_addr: string; remote_port: number; protocol: string } }
  | { kind: "dns_query"; detail: { domain: string } }
  | { kind: "persistence"; detail: { path: string; action: string } };

export interface LiveEvent {
  id: number | null;
  session_id: number | null;
  pid: number;
  parent_pid: number | null;
  ts: number;
  detail: EventDetail;
}

export interface EventRow {
  id: number;
  pid: number;
  parent_pid: number | null;
  ts: number;
  kind: EventKind;
  detail_json: string;
}

export interface Finding {
  id: number | null;
  event_id: number | null;
  rule_id: string;
  severity: Severity;
  message: string;
  status: FindingStatus;
  notes: string | null;
  created_at: number;
  pid: number;
  event_kind: string;
  event_summary: string;
}

export interface Status {
  events_total: number;
  findings_open: number;
  findings_total: number;
  helper_connected: boolean;
  monitored_pid_count: number;
  target_count: number;
  /** "show_all" (no enabled targets) or "filter" (≥1 enabled target). */
  mode: "show_all" | "filter";
}

export type TargetMatchKind =
  | "exe_path"
  | "exe_prefix"
  | "process_name"
  | "bundle_id";

export interface Target {
  id: number | null;
  name: string;
  match_kind: TargetMatchKind;
  match_value: string;
  enabled: boolean;
  created_at: number;
}

export interface TargetStatus {
  targets: Target[];
  monitored_pid_count: number;
  mode: "show_all" | "filter";
}

// ── Commands ────────────────────────────────────────────

export async function fetchRecentEvents(limit = 200): Promise<EventRow[]> {
  return invoke<EventRow[]>("recent_events", { limit });
}

export async function fetchStatus(): Promise<Status> {
  return invoke<Status>("status");
}

export async function fetchFindings(
  limit = 200,
  status?: FindingStatus,
): Promise<Finding[]> {
  return invoke<Finding[]>("list_findings", { limit, status: status ?? null });
}

export async function setFindingStatus(
  id: number,
  newStatus: FindingStatus,
  notes?: string,
): Promise<void> {
  return invoke("update_finding_status", {
    id,
    newStatus,
    notes: notes ?? null,
  });
}

// ── Targets ─────────────────────────────────────────────

export async function listTargets(): Promise<TargetStatus> {
  return invoke<TargetStatus>("list_targets");
}

export async function addTarget(
  name: string,
  matchKind: TargetMatchKind,
  matchValue: string,
): Promise<number> {
  return invoke<number>("add_target", { name, matchKind, matchValue });
}

export async function removeTarget(id: number): Promise<void> {
  return invoke("remove_target", { id });
}

export async function toggleTarget(id: number, enabled: boolean): Promise<void> {
  return invoke("toggle_target", { id, enabled });
}

// ── Dashboard ───────────────────────────────────────────

export interface SeverityBucket { severity: Severity; count: number; }
export interface TopRule { rule_id: string; severity: Severity; count: number; }
export interface HourBucket { hour_start: number; count: number; }
export interface KindBucket { kind: EventKind | string; count: number; }

export interface DashboardSummary {
  total_events: number;
  total_findings: number;
  events_last_hour: number;
  events_last_24h: number;
  findings_last_hour: number;
  findings_last_24h: number;
  by_severity: SeverityBucket[];
  top_rules: TopRule[];
  events_by_hour_24h: HourBucket[];
  events_by_kind_24h: KindBucket[];
}

export async function fetchDashboard(): Promise<DashboardSummary> {
  return invoke<DashboardSummary>("dashboard_summary");
}

// ── Reports & settings ──────────────────────────────────

export interface DataStats {
  db_bytes: number;
  events_total: number;
  findings_total: number;
  oldest_event_ts: number | null;
}

export async function exportReport(
  startTsNs: number,
  endTsNs: number,
  severity?: Severity,
): Promise<string> {
  return invoke<string>("export_report", {
    startTsNs,
    endTsNs,
    severity: severity ?? null,
  });
}

export async function getDataStats(): Promise<DataStats> {
  return invoke<DataStats>("get_data_stats");
}

export async function clearOldEvents(days: number): Promise<number> {
  return invoke<number>("clear_old_events", { days });
}

export async function clearAllData(): Promise<void> {
  return invoke("clear_all_data");
}

// ── Rules CRUD ─────────────────────────────────────────

export type RuleType = "file" | "process" | "network" | "correlation" | "metric";
export type RuleSource = "builtin" | "user";

export interface RuleWithSource {
  id: string;
  type: RuleType;
  severity: Severity;
  message: string | null;
  pattern: string | null;
  action: string | null;
  cmd_pattern: string | null;
  domains: string[] | null;
  require_all_rules: string[] | null;
  window_seconds: number | null;
  metric: string | null;
  threshold: number | null;
  enabled: boolean;
  source: RuleSource;
}

export async function listAllRules(): Promise<RuleWithSource[]> {
  return invoke<RuleWithSource[]>("list_all_rules");
}

export async function upsertUserRule(yamlBody: string): Promise<void> {
  return invoke("upsert_user_rule", { yamlBody });
}

export async function deleteUserRule(id: string): Promise<void> {
  return invoke("delete_user_rule", { id });
}

export async function setRuleEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke("set_rule_enabled", { id, enabled });
}

// ── Event channel ───────────────────────────────────────

export async function onLiveEvent(
  handler: (ev: LiveEvent) => void,
): Promise<UnlistenFn> {
  return listen<LiveEvent>("event", (msg) => handler(msg.payload));
}

export async function onLiveFinding(
  handler: (f: Finding) => void,
): Promise<UnlistenFn> {
  return listen<Finding>("finding", (msg) => handler(msg.payload));
}

// ── Display helpers ─────────────────────────────────────

export function summarize(ev: LiveEvent | EventRow): string {
  if ("detail" in ev) {
    switch (ev.detail.kind) {
      case "process_spawn": {
        const { exe, cmdline } = ev.detail.detail;
        return cmdline || exe || "(empty cmdline)";
      }
      case "process_exit":
        return `exited (code=${ev.detail.detail.exit_code ?? "?"})`;
      case "file_read":
        return `read ${ev.detail.detail.path}`;
      case "file_write":
        return `wrote ${ev.detail.detail.path}`;
      case "file_delete":
        return `deleted ${ev.detail.detail.path}`;
      case "net_connect":
        return `connect ${ev.detail.detail.remote_addr}:${ev.detail.detail.remote_port}`;
      case "dns_query":
        return `dns ${ev.detail.detail.domain}`;
      case "persistence":
        return `${ev.detail.detail.action} ${ev.detail.detail.path}`;
    }
  }
  return ev.detail_json.slice(0, 120);
}

export function eventKind(ev: LiveEvent | EventRow): EventKind {
  if ("detail" in ev) return ev.detail.kind;
  return ev.kind;
}
