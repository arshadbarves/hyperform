import { isAgentDone } from "./agentTree";
import { MAIN_AGENT, type SessionView } from "./sessionView/types";

/**
 * The right-hand side panel: which tabs it has, what it does when new work
 * arrives in the open chat, and how wide it may dock. Pure; the ui store
 * (`lib/stores/ui.svelte.ts`) holds its state.
 */

/** The tabs: the chat's changes, its plan, the agents and jobs working for it, and what fills its context window. */
export type PanelTab = "changes" | "plan" | "agents" | "context";

/** Planned, not built: it keeps its place in the order and stays hidden until it exists. */
export type ReservedPanelTab = "browser";

export interface PanelTabSpec {
  id: PanelTab | ReservedPanelTab;
  label: string;
  available: boolean;
}

export const PANEL_TAB_SPECS: readonly PanelTabSpec[] = [
  { id: "changes", label: "Changes", available: true },
  { id: "plan", label: "Plan", available: true },
  { id: "agents", label: "Agents", available: true },
  { id: "context", label: "Context", available: true },
  { id: "browser", label: "Browser", available: false },
];

export interface PanelTabItem {
  id: PanelTab;
  label: string;
}

/** The tabs the strip shows, in order. */
export function panelTabs(): PanelTabItem[] {
  return PANEL_TAB_SPECS.filter((spec) => spec.available && isPanelTab(spec.id)).map((spec) => ({
    id: spec.id as PanelTab,
    label: spec.label,
  }));
}

export function isPanelTab(value: string): value is PanelTab {
  return value === "changes" || value === "plan" || value === "agents" || value === "context";
}

/** The shown tab `dir` steps from `tab` (arrow keys in the strip), wrapping at the ends. */
export function stepTab(tab: PanelTab, dir: -1 | 1): PanelTab {
  const ids = panelTabs().map((t) => t.id);
  const at = Math.max(0, ids.indexOf(tab));
  return ids[(at + dir + ids.length) % ids.length] ?? tab;
}

export const PANEL_MIN_W = 340;
export const PANEL_DEFAULT_W = 460;
export const PANEL_MAX_W = 1100;
/** What the chat keeps beside a docked panel, the gap between them included. */
export const STAGE_MIN_W = 400;

/**
 * A docked width that fits `room` (the stage and the panel together): no
 * narrower than PANEL_MIN_W, no wider than PANEL_MAX_W or than what leaves
 * the chat STAGE_MIN_W. When the room is too small the panel keeps its minimum.
 */
export function clampPanelWidth(width: number, room = Number.POSITIVE_INFINITY): number {
  const wanted = Number.isFinite(width) ? width : PANEL_DEFAULT_W;
  const upper = Math.min(PANEL_MAX_W, room - STAGE_MIN_W);
  return Math.round(Math.max(PANEL_MIN_W, Math.min(wanted, upper)));
}

/** What the panel has seen of the open chat: its pending plans and its running helpers. */
export interface PanelWatch {
  sessionId: string | null;
  plans: readonly string[];
  agents: readonly string[];
}

export const EMPTY_WATCH: PanelWatch = { sessionId: null, plans: [], agents: [] };

export function watchOf(view: Pick<SessionView, "sessionId" | "plans" | "agents"> | null): PanelWatch {
  if (!view) return EMPTY_WATCH;
  const agents = Object.values(view.agents)
    .filter((a) => a.agentId !== MAIN_AGENT && !isAgentDone(a.status))
    .map((a) => a.agentId);
  return { sessionId: view.sessionId, plans: Object.keys(view.plans), agents };
}

export interface PanelNudge {
  /** A tab to bring forward: a plan is waiting for review. */
  open: PanelTab | null;
  /** Tabs to mark without taking focus: a helper started. */
  badge: PanelTab[];
}

/**
 * What new work in the open chat does to the panel. A plan that becomes
 * pending opens the Plan tab; a helper that starts only badges Agents, and
 * nothing is news for a tab already showing. Switching chats is not news.
 */
export function panelNudge(before: PanelWatch, now: PanelWatch, showing: PanelTab | null): PanelNudge {
  if (now.sessionId === null || before.sessionId !== now.sessionId) return { open: null, badge: [] };
  const newPlan = now.plans.some((id) => !before.plans.includes(id));
  const newAgent = now.agents.some((id) => !before.agents.includes(id));
  return {
    open: newPlan && showing !== "plan" ? "plan" : null,
    badge: newAgent && showing !== "agents" ? ["agents"] : [],
  };
}
