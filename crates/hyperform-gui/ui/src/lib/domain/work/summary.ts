import type { PlanStatus } from "../../protocol/PlanStatus";
import type { PlanSummary } from "../../protocol/PlanSummary";
import type { TaskTone } from "./status";

const STATUS: Record<PlanStatus, [string, TaskTone]> = {
  draft: ["Draft", "neutral"],
  active: ["Building", "working"],
  replanning: ["Replanning", "attention"],
  paused: ["Paused", "quiet"],
  done: ["Done", "ok"],
  abandoned: ["Abandoned", "quiet"],
};

/** A listed plan's status word and tone, as its own Plan tab says it. */
export function summaryStatus(status: PlanStatus): { label: string; tone: TaskTone } {
  const [label, tone] = STATUS[status];
  return { label, tone };
}

/** "2 of 5 done". */
export function summaryProgress(plan: PlanSummary): string {
  return `${plan.tasksDone} of ${plan.tasksTotal} done`;
}

/** "3 things need you"; null when nothing does. */
export function needsLine(count: number): string | null {
  if (count <= 0) return null;
  return count === 1 ? "1 thing needs you" : `${count} things need you`;
}

/** Work goes on: building, waiting on a plan change, or paused. */
export function isContinuable(status: PlanStatus): boolean {
  return status === "active" || status === "paused" || status === "replanning";
}

/** Finished with: done or abandoned. */
export function isSettled(status: PlanStatus): boolean {
  return status === "done" || status === "abandoned";
}

function existing(id: string | null, chats: readonly { sessionId: string }[]): string | null {
  return id && chats.some((c) => c.sessionId === id) ? id : null;
}

/** The chat a plan was last attached to, when that chat still exists: where Continue and the Inbox reopen it. */
export function liveChat(plan: PlanSummary, chats: readonly { sessionId: string }[]): string | null {
  return existing(plan.lastSessionId, chats);
}

/** The chat a plan is attached to right now, when that chat still exists; null once it was detached. */
export function attachedChat(plan: PlanSummary, chats: readonly { sessionId: string }[]): string | null {
  return existing(plan.attachedTo, chats);
}
