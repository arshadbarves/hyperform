import type { Entry } from "../../protocol/Entry";
import type { Task } from "../../protocol/Task";
import type { WorkView } from "../sessionView/work";
import { isClosed, type TaskMap } from "./status";

export type ChangeKind = "added" | "reopened" | "replaced" | "removed" | "changed";

export interface TaskChange {
  task: Task;
  kind: ChangeKind;
}

export interface ChangeSet {
  /** The revision the changes are counted from. */
  since: number;
  tasks: TaskChange[];
  /** `PlanChange` entries recorded since, newest first. */
  changes: Entry[];
  /** "2 tasks added, 1 reopened, 1 replaced". */
  line: string;
}

const ORDER: readonly ChangeKind[] = ["added", "reopened", "replaced", "removed", "changed"];

const WORD: Record<ChangeKind, string> = {
  added: "added",
  reopened: "reopened",
  replaced: "replaced",
  removed: "removed",
  changed: "changed",
};

/**
 * The revision you last approved: the approval seen live, else the move into
 * replanning, else the revision before the newest plan change, else now.
 */
export function replanBaseline(work: WorkView): number {
  const moves = work.moves;
  for (let i = moves.length - 1; i >= 0; i--) {
    if (moves[i].to === "active") return moves[i].revision;
    if (moves[i].to === "replanning") return moves[i].revision - 1;
  }
  const latest = Object.values(work.entries)
    .filter((e) => e.body.type === "planChange")
    .reduce((max, e) => Math.max(max, e.createdRevision), 0);
  return latest > 0 ? latest - 1 : work.plan.revision;
}

/** How a task changed after `since`; null when it did not. */
export function changeKind(task: Task, since: number): ChangeKind | null {
  if (task.revision <= since) return null;
  if (task.status.type === "superseded") return "replaced";
  if (task.status.type === "cancelled") return "removed";
  if (task.createdRevision > since) return "added";
  const hadWork = task.result !== null || task.acceptance.some((c) => c.evidence.length > 0);
  return hadWork && !isClosed(task.status) ? "reopened" : "changed";
}

function phrase(kind: ChangeKind, n: number, first: boolean): string {
  return first ? `${n} ${n === 1 ? "task" : "tasks"} ${WORD[kind]}` : `${n} ${WORD[kind]}`;
}

/** "2 tasks added, 1 reopened"; empty when nothing changed. */
export function changeLine(changes: readonly TaskChange[]): string {
  const counts = new Map<ChangeKind, number>();
  for (const c of changes) counts.set(c.kind, (counts.get(c.kind) ?? 0) + 1);
  const kinds = ORDER.filter((k) => counts.has(k));
  return kinds.map((k, i) => phrase(k, counts.get(k) ?? 0, i === 0)).join(", ");
}

export function changeSet(work: WorkView, since = replanBaseline(work)): ChangeSet {
  const tasks = Object.values(work.tasks)
    .flatMap((task) => {
      const kind = changeKind(task, since);
      return kind ? [{ task, kind }] : [];
    })
    .sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind) || a.task.order - b.task.order);
  const changes = Object.values(work.entries)
    .filter((e) => e.body.type === "planChange" && e.createdRevision > since)
    .sort((a, b) => b.createdAt - a.createdAt);
  return { since, tasks, changes, line: changeLine(tasks) };
}

/** What one plan change did to its tasks: "2 tasks added · 1 replaced". */
export function changeEffect(entry: Entry, tasks: TaskMap): string {
  if (entry.body.type !== "planChange") return "";
  const since = entry.createdRevision - 1;
  const changes = entry.body.affected.flatMap((id) => {
    const task = tasks[id];
    if (!task) return [];
    return [{ task, kind: changeKind(task, since) ?? ("changed" as const) }];
  });
  return changeLine(changes).replaceAll(", ", " · ");
}
