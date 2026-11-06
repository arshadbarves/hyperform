import type { CtxMeter } from "./contextMeter";
import type { LiveStatus } from "./liveStatus";
import { MAIN_AGENT, type SessionView, type ToolCallView } from "./sessionView/types";
import { todoProgress } from "./todos";
import { activityLabel } from "./tools/activityLabel";
import { planRow } from "./work/progress";

/** The compact island: one line of words and at most one number. */
export interface IslandLine {
  text: string | null;
  metric: string | null;
}

export function islandLine(status: LiveStatus, chatTitle: string | null): IslandLine {
  const metric =
    status.kind === "working" || status.kind === "done"
      ? status.elapsed
      : status.kind === "retrying"
        ? status.detail
        : null;
  return { text: status.text ?? chatTitle, metric };
}

/**
 * The island's four shapes: `rest` (the pet and the chat's title), `live`
 * (a step, retry, notice or result, with its clock and progress), `alert`
 * (this chat needs you) and `expanded` (the card).
 */
export type IslandMode = "rest" | "live" | "alert" | "expanded";

export function islandMode(status: Pick<LiveStatus, "kind">, expanded: boolean): IslandMode {
  if (expanded) return "expanded";
  if (status.kind === "attention") return "alert";
  return status.kind === "idle" ? "rest" : "live";
}

export interface IslandAction {
  label: string;
  /** The accessible name: the button goes to the waiting card, it never answers for you. */
  hint: string;
}

/** The alert's one inline button. */
export function islandAction(status: Pick<LiveStatus, "kind" | "activity">): IslandAction | null {
  if (status.kind !== "attention") return null;
  if (status.activity === "question") return { label: "Answer", hint: "Go to the question" };
  if (status.activity === "plan") return { label: "Review", hint: "Go to the plan" };
  return { label: "Approve", hint: "Go to the approval request" };
}

/** Capsule height in pixels; `--island-h` in tokens.css must match. */
export const ISLAND_H = 30;
/** The expanded card's header: the capsule row, inset so the corners clear the pet. */
export const ISLAND_HEAD_H = 44;
const CARD_W = 400;
const CARD_MAX_H = 560;
const CARD_RADIUS = 22;
/** Room kept free beside the card and below it. */
const EDGE_GAP = 12;
const BELOW_GAP = 80;

export interface IslandRoom {
  /** The capsule's width: its words' natural width, clamped to the title bar's room. */
  capsule: number;
  /** The card body's natural height. */
  card: number;
  viewportW: number;
  viewportH: number;
}

export interface IslandShape {
  width: number;
  height: number;
  radius: number;
}

/** The card's fixed width and how tall its body may grow before it scrolls. */
export function islandCard(room: Pick<IslandRoom, "viewportW" | "viewportH">): { width: number; bodyMax: number } {
  const width = Math.max(ISLAND_H, Math.min(CARD_W, room.viewportW - EDGE_GAP * 2));
  const maxHeight = Math.max(ISLAND_HEAD_H, Math.min(CARD_MAX_H, room.viewportH - BELOW_GAP));
  return { width, bodyMax: maxHeight - ISLAND_HEAD_H };
}

/** The surface's size and corner radius for a mode; the stylesheet springs between them. */
export function islandShape(mode: IslandMode, room: IslandRoom): IslandShape {
  if (mode !== "expanded") return { width: Math.max(ISLAND_H, room.capsule), height: ISLAND_H, radius: ISLAND_H / 2 };
  const card = islandCard(room);
  return { width: card.width, height: ISLAND_HEAD_H + Math.min(card.bodyMax, room.card), radius: CARD_RADIUS };
}

/** A notice opens the island by itself when the line cannot hold it, it is an error, or it offers actions. */
export function noticeOpensIsland(notice: { tone: string; actions?: unknown[] } | null, truncated: boolean): boolean {
  if (!notice) return false;
  return truncated || notice.tone === "error" || (notice.actions?.length ?? 0) > 0;
}

export interface ContextBubbleLook {
  tone: "quiet" | "warn" | "danger";
  /** The percentage, shown only once the context is filling up. */
  label: string | null;
}

export function contextBubble(meter: Pick<CtxMeter, "pct" | "level">): ContextBubbleLook {
  if (meter.level === "ok") return { tone: "quiet", label: null };
  return { tone: meter.level, label: `${meter.pct}%` };
}

export interface IslandPlanRow {
  done: number;
  total: number;
  /** "4 of 9 · Replay on reconnect". */
  label: string;
}

/** The card's Plan row: the attached plan's tasks, else the main agent's todos; null when there is none. */
export function islandPlanRow(view: Pick<SessionView, "sessionId" | "todos" | "work"> | null): IslandPlanRow | null {
  if (view?.work) {
    const row = planRow(view.work, view.sessionId);
    return row.progress.total ? { ...row.progress, label: row.label } : null;
  }
  const todos = todoProgress(view?.todos[MAIN_AGENT]);
  if (!todos.total) return null;
  const current = todos.currentLabel && !todos.allDone ? ` · ${todos.currentLabel}` : "";
  return { done: todos.done, total: todos.total, label: `${todos.done} of ${todos.total}${current}` };
}

export interface IslandStep {
  callId: string;
  label: string;
  state: "running" | "done" | "failed";
}

/** The main agent's latest steps since `since`, newest first, for the expanded island. */
export function recentSteps(tools: Record<string, ToolCallView>, since: number, limit: number): IslandStep[] {
  return Object.values(tools)
    .filter((t) => t.agentId === MAIN_AGENT && t.startedAt >= since)
    .sort((a, b) => b.startedAt - a.startedAt)
    .slice(0, limit)
    .map((t) => ({
      callId: t.callId,
      label: activityLabel(t.tool, t.input),
      state: t.status === "running" ? "running" : t.status === "ok" ? "done" : "failed",
    }));
}
