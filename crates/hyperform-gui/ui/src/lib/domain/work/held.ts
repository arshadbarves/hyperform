import type { Task } from "../../protocol/Task";
import type { WorkEdit } from "../../protocol/WorkEdit";
import type { WorkView } from "../sessionView/work";
import { isClosed } from "./status";

/** Hold a task (it never starts while held; a running helper stops at its next step) or release it again. */
export function holdEdit(taskId: string, hold: boolean): WorkEdit {
  return hold ? { type: "holdTask", taskId, held: true } : { type: "releaseTask", taskId };
}

/** A row's quiet hold badge and its tooltip. */
export interface HoldBadge {
  text: string;
  title: string;
  /** Held by you, rather than by the plan change. */
  yours: boolean;
}

export const ON_HOLD: HoldBadge = {
  text: "on hold",
  title: "You hold it: it won't start until you release it",
  yours: true,
};

export const WAITS_FOR_CHANGE: HoldBadge = {
  text: "waits for the plan change",
  title: "Held until you accept the plan change; the other ready tasks keep running",
  yours: false,
};

/**
 * The open tasks the plan change holds back while the plan is replanning
 * (after a failure or an overlap), in plan order; empty otherwise.
 */
export function replanHeldTasks(work: WorkView): Task[] {
  if (work.plan.status !== "replanning") return [];
  return work.replanHeld
    .flatMap((id) => {
      const task = work.tasks[id];
      return task && !isClosed(task.status) ? [task] : [];
    })
    .sort((a, b) => a.order - b.order || a.createdAt - b.createdAt);
}

/** Badges by task id; your own hold wins, since it outlasts the plan change. */
export function holdBadges(work: WorkView): Record<string, HoldBadge> {
  const out: Record<string, HoldBadge> = {};
  for (const task of replanHeldTasks(work)) out[task.id] = WAITS_FOR_CHANGE;
  for (const task of Object.values(work.tasks)) {
    if (task.held && !isClosed(task.status)) out[task.id] = ON_HOLD;
  }
  return out;
}

/** "2 tasks wait for you to accept the plan change"; null when none does. */
export function replanWaitLine(count: number): string | null {
  if (count <= 0) return null;
  return count === 1
    ? "1 task waits for you to accept the plan change"
    : `${count} tasks wait for you to accept the plan change`;
}
