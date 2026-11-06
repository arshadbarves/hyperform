import type { Task } from "../../protocol/Task";
import type { WorkView } from "../sessionView/work";
import { MAIN_AGENT } from "../sessionView/types";
import { todoProgress } from "../todos";
import type { TaskMap } from "./status";

export interface Progress {
  done: number;
  total: number;
}

/** Tasks that count toward progress: not cancelled, not replaced, not merely proposed. */
function counts(task: Task): boolean {
  const type = task.status.type;
  return type !== "cancelled" && type !== "superseded" && type !== "proposed";
}

/** Done of total; a check mark means the kernel accepted the evidence, not that an agent said so. */
export function planProgress(tasks: TaskMap | readonly Task[]): Progress {
  const list = (Array.isArray(tasks) ? tasks : Object.values(tasks)) as readonly Task[];
  const counted = list.filter(counts);
  return { done: counted.filter((t) => t.status.type === "done").length, total: counted.length };
}

/**
 * The task being worked on now: the one bound to this session's main
 * agent, else any active task, else one being verified.
 */
export function currentTask(work: WorkView | null, sessionId: string | null): Task | null {
  if (!work) return null;
  const tasks = Object.values(work.tasks);
  const active = tasks.filter((t) => t.status.type === "active");
  return (
    active.find((t) => t.owner?.agentId === MAIN_AGENT && t.owner.sessionId === sessionId) ??
    active[0] ??
    tasks.find((t) => t.status.type === "verifying") ??
    null
  );
}

/** The task's step in progress (its `TodoWrite` micro-plan), else what the task itself is doing. */
export function stepLabel(task: Task): string {
  const steps = todoProgress(task.steps);
  if (steps.currentLabel && !steps.allDone) return steps.currentLabel;
  return task.activeForm || task.title;
}

export function stepProgress(task: Task): Progress {
  const steps = todoProgress(task.steps);
  return { done: steps.done, total: steps.total };
}

/** Criteria met or waived, of all. */
export function criteriaMet(task: Task): Progress {
  const done = task.acceptance.filter((c) => c.status === "met" || c.status === "waived").length;
  return { done, total: task.acceptance.length };
}

/** When each done task last changed, for counting what a turn finished. */
export function doneTimes(tasks: TaskMap): number[] {
  return Object.values(tasks)
    .filter((t) => t.status.type === "done")
    .map((t) => t.updatedAt);
}

/** Tasks that became done between `from` and `to` (a turn's start and end). */
export function tasksDoneIn(times: readonly number[], from: number, to: number): number {
  return times.filter((at) => at >= from && at <= to).length;
}

/** The island card's Plan row: "4 of 9 · Replay on reconnect". */
export function planRow(work: WorkView, sessionId: string | null): { progress: Progress; label: string } {
  const progress = planProgress(work.tasks);
  const current = currentTask(work, sessionId);
  const label = `${progress.done} of ${progress.total}${current ? ` · ${current.title}` : ""}`;
  return { progress, label };
}
