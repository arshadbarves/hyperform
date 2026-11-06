import type { BlockReason } from "../../protocol/BlockReason";
import type { PlanStatus } from "../../protocol/PlanStatus";
import type { Task } from "../../protocol/Task";
import type { TaskStatus } from "../../protocol/TaskStatus";

/**
 * What a task row shows: the stored status, plus the derived `ready`
 * (pending, every dependency done, not held, the plan active), `waiting`
 * (pending with open dependencies) and `held`.
 */
export type TaskState =
  | "proposed"
  | "pending"
  | "ready"
  | "waiting"
  | "held"
  | "active"
  | "verifying"
  | "done"
  | "blocked"
  | "failed"
  | "cancelled"
  | "superseded";

/** A status tone: green live work, amber needs you, red failed; quiet for what is settled or idle. */
export type TaskTone = "quiet" | "neutral" | "working" | "ok" | "attention" | "danger";

export interface TaskLook {
  state: TaskState;
  tone: TaskTone;
  label: string;
  /** Why it is in that state: the failure, the blocker, what it waits on. */
  detail: string | null;
}

const LABEL: Record<TaskState, string> = {
  proposed: "Proposed",
  pending: "Pending",
  ready: "Ready",
  waiting: "Waiting",
  held: "On hold",
  active: "In progress",
  verifying: "Verifying",
  done: "Done",
  blocked: "Blocked",
  failed: "Failed",
  cancelled: "Not needed",
  superseded: "Replaced",
};

const TONE: Record<TaskState, TaskTone> = {
  proposed: "attention",
  pending: "neutral",
  ready: "neutral",
  waiting: "quiet",
  held: "quiet",
  active: "working",
  verifying: "working",
  done: "ok",
  blocked: "attention",
  failed: "danger",
  cancelled: "quiet",
  superseded: "quiet",
};

export type TaskMap = Readonly<Record<string, Task>>;

/** Done, not needed or replaced: nothing more happens to it. */
export function isClosed(status: TaskStatus): boolean {
  return status.type === "done" || status.type === "cancelled" || status.type === "superseded";
}

function title(tasks: TaskMap, id: string): string {
  return tasks[id]?.title ?? id;
}

/** Dependencies that are known and not done yet. */
export function openDependencies(task: Task, tasks: TaskMap): Task[] {
  return task.dependsOn.flatMap((id) => {
    const dep = tasks[id];
    return dep && dep.status.type !== "done" ? [dep] : [];
  });
}

export function taskState(task: Task, tasks: TaskMap, plan: PlanStatus): TaskState {
  const type = task.status.type;
  if (type !== "pending") return type;
  if (task.held) return "held";
  if (openDependencies(task, tasks).length > 0) return "waiting";
  return plan === "active" ? "ready" : "pending";
}

export function blockedText(reason: BlockReason, tasks: TaskMap): string {
  switch (reason.type) {
    case "dependency":
      return `waits on “${title(tasks, reason.taskId)}”`;
    case "dependencySuperseded":
      return `depends on “${title(tasks, reason.taskId)}”, which was replaced by “${title(tasks, reason.by)}”`;
    case "mergeConflict":
      return reason.detail ? `merge conflict · ${reason.detail}` : "merge conflict";
    case "unreported":
      return "ended without a report";
    case "changeDiscarded":
      return "its change was discarded";
    case "other":
      return reason.text;
  }
  return unknownBlockText(reason as unknown as Readonly<Record<string, unknown>>);
}

/** A block reason from a newer engine than this build: its own words, else its name. */
function unknownBlockText(reason: Readonly<Record<string, unknown>>): string {
  for (const key of ["text", "detail", "reason"]) {
    const value = reason[key];
    if (typeof value === "string" && value.trim()) return value;
  }
  const type = typeof reason.type === "string" ? reason.type : "";
  if (/discard/i.test(type)) return "its change was discarded";
  return type.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase() || "blocked";
}

/**
 * What a blocked task waits on: another task (`dependency`, which the
 * kernel clears), a merge conflict, or you (`retry`: it ended without a
 * report, its change was discarded at the merge card, or a reason this
 * build does not know).
 */
export type BlockKind = "dependency" | "conflict" | "retry" | "other";

export function blockKind(reason: BlockReason): BlockKind {
  switch (reason.type) {
    case "dependency":
    case "dependencySuperseded":
      return "dependency";
    case "mergeConflict":
      return "conflict";
    case "other":
      return "other";
    default:
      return "retry";
  }
}

/** "after Research" for one open dependency, "waits on 2" for more; nothing once they are done. */
export function dependencyBadge(task: Task, tasks: TaskMap): string | null {
  if (isClosed(task.status)) return null;
  const open = openDependencies(task, tasks);
  if (open.length === 0) return null;
  return open.length === 1 ? `after ${open[0].title}` : `waits on ${open.length}`;
}

/** A user-checked criterion waits on you while the task is verifying, or the task itself is only proposed. */
export function needsYou(task: Task): boolean {
  if (task.status.type === "proposed") return true;
  return (
    task.status.type === "verifying" &&
    task.acceptance.some((c) => c.method.type === "user" && c.status === "pending")
  );
}

/** Failed, blocked and waiting-on-you tasks are never folded out of sight. */
export function isPinned(task: Task): boolean {
  return task.status.type === "failed" || task.status.type === "blocked" || needsYou(task);
}

function detail(task: Task, tasks: TaskMap, state: TaskState): string | null {
  const status = task.status;
  if (status.type === "failed") return status.reason || null;
  if (status.type === "blocked") return blockedText(status.reason, tasks);
  if (status.type === "superseded") return `replaced by “${title(tasks, status.by)}”`;
  if (state === "waiting") return dependencyBadge(task, tasks);
  if (needsYou(task) && status.type === "verifying") return "waits on your check";
  return null;
}

export function taskLook(task: Task, tasks: TaskMap, plan: PlanStatus): TaskLook {
  const state = taskState(task, tasks, plan);
  const tone = needsYou(task) ? "attention" : TONE[state];
  return { state, tone, label: LABEL[state], detail: detail(task, tasks, state) };
}

export function stateLabel(state: TaskState): string {
  return LABEL[state];
}

/** What you may do to a task from its detail; nothing while the plan is read-only. */
export interface TaskActions {
  rename: boolean;
  hold: boolean;
  /** You held it: release it so it can start again. */
  release: boolean;
  retry: boolean;
  cancel: boolean;
  runAlone: boolean;
  /** A proposed task waits for you: Accept makes it pending, Dismiss drops it. */
  accept: boolean;
}

export function taskActions(task: Task, state: TaskState, readOnly: boolean): TaskActions {
  const open = !readOnly && !isClosed(task.status);
  const waiting = state === "pending" || state === "ready" || state === "waiting" || state === "proposed";
  return {
    rename: open,
    hold: open && !task.held && (waiting || state === "active"),
    release: open && task.held,
    retry: open && (state === "failed" || state === "blocked"),
    cancel: open && state !== "proposed",
    runAlone: open,
    accept: open && state === "proposed",
  };
}

/** Added by a turn that read web content (a task or a notebook entry). */
export function fromWeb(item: Pick<Task, "provenance">): boolean {
  return item.provenance === "research";
}

/** The tooltip of the "from web" mark. */
export const WEB_HINT = "Added by a turn that read web content";

const KIND: Record<Task["kind"], string> = {
  research: "Research",
  design: "Design",
  implement: "Implement",
  verify: "Verify",
  integrate: "Integrate",
  experiment: "Experiment",
};

export function kindLabel(kind: Task["kind"]): string {
  return KIND[kind];
}
