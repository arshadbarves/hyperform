import type { PacketSent, WorkView } from "../sessionView/work";
import { MAIN_AGENT, type NoticeView } from "../sessionView/types";
import { changeEffect } from "./changes";
import { isResumeChange, resumeSummary } from "./resume";
import { blockedText, type TaskTone } from "./status";

export type TraceKind = "approved" | "revised" | "resumed" | "failed" | "blocked" | "moved";

/** A quiet one-line trace in the transcript of something the plan did in the background. */
export interface PlanTrace {
  key: string;
  /** Null once the plan left this chat. */
  planId: string | null;
  kind: TraceKind;
  at: number;
  text: string;
  detail: string | null;
  tone: TaskTone;
  /** The task to open, for a failed or blocked task. */
  taskId: string | null;
  /** The plan change behind a revision, for its Why. */
  entryId: string | null;
}

/**
 * Trace rows from the plan: approvals seen live, every recorded plan
 * change (a resume after a restart or an attach reads as such), and tasks
 * that are failed or blocked now. Oldest first.
 */
export function planTraces(work: WorkView): PlanTrace[] {
  const planId = work.plan.planId;
  const out: PlanTrace[] = [];
  for (const move of work.moves) {
    if (move.to !== "active" || (move.from !== "draft" && move.from !== "replanning")) continue;
    out.push({
      key: `trace:approved:${move.revision}`,
      planId,
      kind: "approved",
      at: move.at,
      text: move.from === "draft" ? "Plan approved" : "Plan changes accepted",
      detail: `rev ${move.revision}`,
      tone: "quiet",
      taskId: null,
      entryId: null,
    });
  }
  for (const entry of Object.values(work.entries)) {
    if (entry.body.type !== "planChange" || entry.status.type === "dismissed") continue;
    if (isResumeChange(entry)) {
      out.push({
        key: `trace:resumed:${entry.id}`,
        planId,
        kind: "resumed",
        at: entry.createdAt,
        text: "Plan resumed",
        detail: resumeSummary(entry) || null,
        tone: "quiet",
        taskId: null,
        entryId: entry.id,
      });
      continue;
    }
    const effect = changeEffect(entry, work.tasks);
    out.push({
      key: `trace:revised:${entry.id}`,
      planId,
      kind: "revised",
      at: entry.createdAt,
      text: "Plan revised",
      detail: [`rev ${entry.createdRevision}`, effect].filter(Boolean).join(" · "),
      tone: "quiet",
      taskId: null,
      entryId: entry.id,
    });
  }
  for (const task of Object.values(work.tasks)) {
    const status = task.status;
    if (status.type !== "failed" && status.type !== "blocked") continue;
    const why = status.type === "failed" ? status.reason : blockedText(status.reason, work.tasks);
    out.push({
      key: `trace:${status.type}:${task.id}`,
      planId,
      kind: status.type,
      at: task.updatedAt,
      text: status.type === "failed" ? "Task failed" : "Task blocked",
      detail: [task.title, why].filter(Boolean).join(" · "),
      tone: status.type === "failed" ? "danger" : "attention",
      taskId: task.id,
      entryId: null,
    });
  }
  return out.sort((a, b) => a.at - b.at);
}

/** Word for word the engine's `MOVED_NOTICE` (work/driver.rs), the notice of the chat a plan moved away from. */
export const PLAN_MOVED_NOTICE = "This plan moved to another chat.";

/**
 * Where the plan left this chat for another: the snapshot after a move no
 * longer carries the plan, so its notice is what stays in the transcript.
 */
export function movedTraces(notices: readonly NoticeView[]): PlanTrace[] {
  return notices
    .filter((n) => n.text === PLAN_MOVED_NOTICE)
    .map((n) => ({
      key: `trace:moved:${n.id}`,
      planId: null,
      kind: "moved",
      at: n.at,
      text: PLAN_MOVED_NOTICE,
      detail: null,
      tone: "quiet",
      taskId: null,
      entryId: null,
    }));
}

/** When the main agent was handed its task packet again after a compaction, for "plan restored" on the divider. */
export function restoreTimes(packets: readonly PacketSent[]): number[] {
  return packets.filter((p) => p.agentId === MAIN_AGENT && p.reason === "compaction").map((p) => p.at);
}

/** A packet went to the main agent after this compaction and before the next one. */
export function planRestored(at: number, next: number | null, times: readonly number[]): boolean {
  return times.some((t) => t >= at && (next === null || t < next));
}
