import { fmtCost } from "../util";
import { workCounts } from "./agentTree";
import { fmtDuration } from "./format";
import type { SessionActivity } from "./sessions";
import type { TurnRecord } from "../protocol/TurnRecord";
import { MAIN_AGENT, type SessionView } from "./sessionView/types";
import { todoProgress } from "./todos";
import { activityLabel } from "./tools/activityLabel";
import { toolMeta, type ToolFamily } from "./tools/toolMeta";
import { outcomeNote } from "./verification";
import { currentTask, planProgress, stepLabel } from "./work/progress";

/** How long a finished turn's result stays in the status line. */
export const DONE_FLASH_MS = 3000;

export type LiveTone = "quiet" | "working" | "attention" | "ok" | "danger";
export type LiveKind = "idle" | "attention" | "notice" | "retrying" | "working" | "done" | "recap";
/** What the agent is doing, for the companion's pose. */
export type LiveActivity =
  | ToolFamily
  | "think"
  | "reply"
  | "compact"
  | "verify"
  | "approval"
  | "warn"
  | "retry"
  | "done"
  | "failed"
  | "info";

/** The part of a toast the status line shows. */
export interface LiveNotice {
  id: number;
  text: string;
  title?: string;
  tag?: string;
  tone: "info" | "ok" | "warn" | "error";
}

/** Another chat that is blocked on the user. */
export interface WaitingChat {
  sessionId: string;
  title: string;
}

export interface LiveStatusInput {
  view: SessionView | null;
  waiting: WaitingChat[];
  notice: LiveNotice | null;
  /** The turn whose ending is being announced (see `freshFinish`). */
  flashTurnId: string | null;
  /** When the user left, if they just came back; turns that ended since then are recapped. */
  awaySince: number | null;
  now: number;
}

export interface LiveStatus {
  kind: LiveKind;
  tone: LiveTone;
  activity: LiveActivity | null;
  /** The step, message or result; null while idle. */
  text: string | null;
  detail: string | null;
  progress: { done: number; total: number } | null;
  /** Live clock while working, the turn's duration when it ended. */
  elapsed: string | null;
  /** This turn's cost while working or just ended; the session's while idle. */
  cost: string | null;
  noticeId: number | null;
  /** The first other chat that needs the user, shown beside the title. */
  waiting: WaitingChat | null;
  moreWaiting: number;
  helpers: { running: number; pending: number };
}

type Main = Pick<LiveStatus, "kind" | "tone" | "activity" | "text" | "detail" | "elapsed" | "cost" | "noticeId">;

function main(
  kind: LiveKind,
  tone: LiveTone,
  activity: LiveActivity | null,
  text: string | null,
  extra: Partial<Main> = {},
): Main {
  return { kind, tone, activity, text, detail: null, elapsed: null, cost: null, noticeId: null, ...extra };
}

/** Whole seconds for a live clock: `8s`, `1m 05s`. */
function clock(ms: number): string {
  return ms < 60_000 ? `${Math.max(0, Math.floor(ms / 1000))}s` : fmtDuration(ms);
}

function money(usd: number): string | null {
  return usd > 0 ? fmtCost(usd) : null;
}

function attentionHere(view: SessionView | null): Main | null {
  if (!view) return null;
  const approvals = Object.values(view.approvals);
  const questions = Object.values(view.questions);
  const plans = Object.values(view.plans);
  if (approvals.length) {
    const text = approvals.length > 1 ? `${approvals.length} approvals waiting` : "Needs your approval";
    return main("attention", "attention", "approval", text, { detail: approvals[0].title });
  }
  if (questions.length) {
    return main("attention", "attention", "question", "Question for you", {
      detail: questions[0].questions[0]?.question ?? null,
    });
  }
  if (plans.length) return main("attention", "attention", "plan", "Plan ready for review");
  return view.status === "waiting" ? main("attention", "attention", "approval", "Waiting for you") : null;
}

const NOTICE_LOOK: Record<LiveNotice["tone"], [LiveTone, LiveActivity]> = {
  info: ["quiet", "info"],
  ok: ["ok", "done"],
  warn: ["attention", "warn"],
  error: ["danger", "failed"],
};

function noticeMain(notice: LiveNotice | null): Main | null {
  if (!notice) return null;
  const [tone, activity] = NOTICE_LOOK[notice.tone];
  const detail = notice.tag ?? (notice.title && notice.title !== notice.text ? notice.text : null);
  return main("notice", tone, activity, notice.title ?? notice.text, { detail, noticeId: notice.id });
}

function retrying(view: SessionView | null, now: number): Main | null {
  const retry = view?.retrying;
  if (!retry) return null;
  const secs = Math.max(0, Math.ceil((retry.at + retry.delayMs - now) / 1000));
  return main("retrying", "attention", "retry", "Provider busy", {
    detail: `retry ${retry.attempt} ${secs > 0 ? `in ${secs}s` : "now"}`,
  });
}

function working(view: SessionView | null, now: number): Main | null {
  if (!view || view.status !== "busy") return null;
  const turn = view.activeTurn;
  const extra = {
    elapsed: turn ? clock(now - turn.startedAt) : null,
    cost: turn ? money(view.costUsd - turn.costAtStart) : null,
  };
  if (view.compacting) return main("working", "working", "compact", "Compacting context", extra);
  const running = Object.values(view.tools)
    .filter((t) => t.status === "running" && t.agentId === MAIN_AGENT)
    .sort((a, b) => b.startedAt - a.startedAt);
  if (running.length) {
    const top = running[0];
    const more = running.length > 1 ? ` +${running.length - 1}` : "";
    return main("working", "working", toolMeta(top.tool).family, activityLabel(top.tool, top.input) + more, extra);
  }
  const stream = Object.values(view.streaming).find((s) => s.agentId === MAIN_AGENT);
  if (stream) {
    return main("working", "working", stream.text ? "reply" : "think", stream.text ? "Writing a reply" : "Thinking", extra);
  }
  if (turn && view.verification) return main("working", "working", "verify", "Checking the changes", extra);
  return main("working", "working", "think", "Working", extra);
}

function result(turn: TurnRecord): [LiveTone, LiveActivity, string] {
  const note = outcomeNote(turn.outcome);
  if (note) {
    const failed = note.tone === "err";
    return [failed ? "danger" : "quiet", failed ? "failed" : "done", note.label.split(" · ")[0]];
  }
  switch (turn.verification.status) {
    case "verified":
      return ["ok", "done", "Verified"];
    case "failed":
      return ["danger", "failed", "Checks failed"];
    case "unverified":
      return ["quiet", "done", "Done · not verified"];
    default:
      return ["quiet", "done", "Done"];
  }
}

function finished(view: SessionView | null, flashTurnId: string | null): Main | null {
  if (!view || !flashTurnId || view.status !== "idle") return null;
  const turn = view.turns.find((t) => t.turnId === flashTurnId);
  if (!turn) return null;
  const [tone, activity, text] = result(turn);
  return main("done", tone, activity, text, {
    elapsed: fmtDuration(turn.finishedAt - turn.startedAt) || null,
    cost: money(turn.costUsd),
  });
}

function recap(view: SessionView | null, since: number | null): Main | null {
  if (!view || since === null || view.status !== "idle") return null;
  const ended = view.turns.filter((t) => t.finishedAt > since);
  const last = ended[ended.length - 1];
  if (!last) return null;
  const [tone, activity, text] = result(last);
  return main("recap", tone, activity, `While you were away: ${text}`, {
    detail: ended.length > 1 ? `${ended.length} turns finished` : null,
    cost: money(ended.reduce((sum, t) => sum + t.costUsd, 0)),
  });
}

/**
 * What the title bar says. Highest priority first: this chat needs you, a
 * passing notice, provider retry, work in progress, a turn that just ended,
 * idle. Another waiting chat is reported beside it, never instead of it.
 */
export function liveStatus(input: LiveStatusInput): LiveStatus {
  const { view, now } = input;
  const current =
    attentionHere(view) ??
    noticeMain(input.notice) ??
    retrying(view, now) ??
    working(view, now) ??
    finished(view, input.flashTurnId) ??
    recap(view, input.awaySince) ??
    main("idle", "quiet", null, null, { cost: view ? money(view.costUsd) : null });
  const work = view?.work ?? null;
  const plan = work ? planProgress(work.tasks) : todoProgress(view?.todos[MAIN_AGENT]);
  const progress =
    current.kind === "working" && plan.total > 0 && plan.done < plan.total ? { done: plan.done, total: plan.total } : null;
  const task = current.kind === "working" && current.activity !== "compact" ? currentTask(work, view?.sessionId ?? null) : null;
  const counts = view ? workCounts(view.agents, view.jobs, view.work) : null;
  return {
    ...current,
    ...(task ? { text: stepLabel(task), detail: current.text } : {}),
    progress,
    waiting: input.waiting[0] ?? null,
    moreWaiting: Math.max(0, input.waiting.length - 1),
    helpers: {
      running: counts ? counts.runningAgents + counts.runningJobs : 0,
      pending: counts?.pendingWorktrees ?? 0,
    },
  };
}

/** The last turn and the flash time left when it just ended; opening an old chat announces nothing. */
export function freshFinish(view: SessionView | null, now: number): { turnId: string; remainingMs: number } | null {
  const last = view?.turns[view.turns.length - 1];
  if (!view || !last || view.status !== "idle") return null;
  const remainingMs = DONE_FLASH_MS - (now - last.finishedAt);
  return remainingMs > 0 ? { turnId: last.turnId, remainingMs } : null;
}

export function waitingChats(
  activity: Record<string, SessionActivity>,
  activeId: string | null,
  titleOf: (sessionId: string) => string,
): WaitingChat[] {
  return Object.entries(activity)
    .filter(([id, state]) => state === "approval" && id !== activeId)
    .map(([id]) => ({ sessionId: id, title: titleOf(id) }));
}
