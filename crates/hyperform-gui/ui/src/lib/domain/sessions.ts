import type { Event } from "../protocol/Event";
import type { EventEnvelope } from "../protocol/EventEnvelope";
import type { TurnOutcome } from "../protocol/TurnOutcome";
import type { VerificationOutcome } from "../protocol/VerificationOutcome";
import { emptyView, reduce, type SessionView } from "./sessionView";

export type SessionActivity = "working" | "approval";

/** A turn that finished while its session was in the background. */
export interface UnreadMark {
  outcome: TurnOutcome;
  verification: VerificationOutcome;
  at: number;
}

export interface SessionsState {
  views: Record<string, SessionView>;
  unread: Record<string, UnreadMark>;
}

/** Something the pet grows from; the runtime adds the day it happened. */
export type PetGrowthSignal =
  | { kind: "turn"; turnId: string; completed: boolean; verified: boolean }
  | { kind: "applied"; agentId: string };

export type RuntimeEffect =
  | { kind: "toast"; tone: "info" | "warn" | "error"; text: string }
  | { kind: "shellOutput"; text: string }
  | { kind: "refreshSessions" }
  /** A plan changed or a chat opened in this project: its plan list (status, needs you) may have moved. */
  | { kind: "refreshPlans"; projectRoot: string }
  | { kind: "petGrowth"; signal: PetGrowthSignal };

export function emptySessionsState(): SessionsState {
  return { views: {}, unread: {} };
}

/**
 * Apply one envelope. Events at or below the last applied sequence are
 * duplicates; a snapshot always applies and resets the sequence.
 */
export function applyEnvelope(
  state: SessionsState,
  env: EventEnvelope,
  activeId: string | null,
  now: number,
): SessionsState {
  const prev = state.views[env.sessionId] ?? emptyView(env.sessionId);
  if (env.event.type !== "snapshot" && env.seq <= prev.lastSeq) return state;
  const next: SessionView = { ...reduce(prev, env.event, now), lastSeq: env.seq };
  const views = { ...state.views, [env.sessionId]: next };
  if (env.event.type !== "turnFinished" || env.sessionId === activeId) {
    return { views, unread: state.unread };
  }
  const { outcome, verification } = env.event.turn;
  return { views, unread: { ...state.unread, [env.sessionId]: { outcome, verification, at: now } } };
}

/** A GUI-originated event (such as `/help` output); the engine sequence is untouched. */
export function applyLocalEvent(
  state: SessionsState,
  sessionId: string,
  event: Event,
  now: number,
): SessionsState {
  const prev = state.views[sessionId] ?? emptyView(sessionId);
  return { ...state, views: { ...state.views, [sessionId]: reduce(prev, event, now) } };
}

/** The user answered the trust request; the engine sends no event for "Not now". */
export function dismissTrustRequest(state: SessionsState, sessionId: string): SessionsState {
  const view = state.views[sessionId];
  if (!view?.trustRequest) return state;
  return { ...state, views: { ...state.views, [sessionId]: { ...view, trustRequest: null } } };
}

export function markRead(state: SessionsState, sessionId: string): SessionsState {
  if (!(sessionId in state.unread)) return state;
  const unread = { ...state.unread };
  delete unread[sessionId];
  return { ...state, unread };
}

export function forgetSession(state: SessionsState, sessionId: string): SessionsState {
  const views = { ...state.views };
  delete views[sessionId];
  return markRead({ ...state, views }, sessionId);
}

export function hasPendingInput(view: SessionView): boolean {
  return (
    Object.keys(view.approvals).length > 0 ||
    Object.keys(view.questions).length > 0 ||
    Object.keys(view.plans).length > 0
  );
}

export function sessionActivity(view: SessionView): SessionActivity | null {
  if (view.status === "waiting" || hasPendingInput(view)) return "approval";
  if (view.status === "busy") return "working";
  return null;
}

export function activityMap(views: Record<string, SessionView>): Record<string, SessionActivity> {
  const out: Record<string, SessionActivity> = {};
  for (const [id, view] of Object.entries(views)) {
    const activity = sessionActivity(view);
    if (activity) out[id] = activity;
  }
  return out;
}

const NOTICE_TONE = { info: "info", warn: "warn", error: "error" } as const;

/**
 * Side effects an event asks of the shell (toasts, list refresh, terminal
 * output, the pet's growth). Growth counts every chat, open or not; only
 * live events arrive here, so reopening a chat awards nothing.
 */
export function eventEffects(event: Event, active: boolean): RuntimeEffect[] {
  switch (event.type) {
    case "turnFinished": {
      const { turnId, outcome, verification } = event.turn;
      const signal: PetGrowthSignal = {
        kind: "turn",
        turnId,
        completed: outcome.type === "completed",
        verified: verification.status === "verified",
      };
      return [{ kind: "refreshSessions" }, { kind: "petGrowth", signal }];
    }
    case "agentUpdated":
      return event.info.worktree?.state === "applied"
        ? [{ kind: "petGrowth", signal: { kind: "applied", agentId: event.info.agentId } }]
        : [];
    case "notice":
      return active ? [{ kind: "toast", tone: NOTICE_TONE[event.level], text: event.text }] : [];
    case "hookRan":
      if (!active || !event.blocked) return [];
      return [
        {
          kind: "toast",
          tone: "warn",
          text: `Hook blocked · ${event.hookEvent}${event.message ? `: ${event.message}` : ""}`,
        },
      ];
    case "commandOutput":
      return active && event.name === "shell" ? [{ kind: "shellOutput", text: event.markdown }] : [];
    case "titleChanged":
    case "turnStarted":
      return [{ kind: "refreshSessions" }];
    case "workUpdated":
      return [{ kind: "refreshPlans", projectRoot: event.update.plan.projectRoot }];
    case "snapshot":
      return [{ kind: "refreshPlans", projectRoot: event.snapshot.info.projectRoot }];
    default:
      return [];
  }
}
