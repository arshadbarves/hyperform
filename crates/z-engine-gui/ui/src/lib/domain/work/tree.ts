import type { PlanStatus } from "../../protocol/PlanStatus";
import type { Task } from "../../protocol/Task";
import { planProgress, type Progress } from "./progress";
import { dependencyBadge, isPinned, taskLook, type TaskLook, type TaskMap } from "./status";

export interface TreeRow {
  task: Task;
  depth: number;
  parentId: string | null;
  /** 1-based place among its shown siblings and their count (aria-posinset, aria-setsize). */
  pos: number;
  size: number;
  hasChildren: boolean;
  expanded: boolean;
  /** False while a failed, blocked or waiting-on-you task sits inside: it is never folded away. */
  canCollapse: boolean;
  look: TaskLook;
  /** "after Research" or "waits on 2". */
  deps: string | null;
  /** Its children, done of total. */
  progress: Progress | null;
}

export interface TreeInput {
  tasks: TaskMap;
  plan: PlanStatus;
  /** Rows opened (true) or closed (false) by hand. */
  expanded: Readonly<Record<string, boolean>>;
  showReplaced: boolean;
  /** Under review (draft, replanning): every group starts open. Otherwise only the path to the active task. */
  review: boolean;
}

export interface TaskTreeView {
  rows: TreeRow[];
  /** Replaced tasks hidden behind "Show N replaced". */
  replaced: number;
}

function byOrder(a: Task, b: Task): number {
  return a.order - b.order || a.createdAt - b.createdAt || a.id.localeCompare(b.id);
}

/** Children per parent id (null for the roots), in sibling order. A missing parent makes a task a root. */
export function childrenOf(tasks: TaskMap): Map<string | null, Task[]> {
  const out = new Map<string | null, Task[]>();
  for (const task of Object.values(tasks)) {
    const parent = task.parent && tasks[task.parent] ? task.parent : null;
    const list = out.get(parent);
    if (list) list.push(task);
    else out.set(parent, [task]);
  }
  for (const list of out.values()) list.sort(byOrder);
  return out;
}

/** From the root down to the task's parent, for the detail's breadcrumb. */
export function ancestors(taskId: string, tasks: TaskMap): Task[] {
  const out: Task[] = [];
  const seen = new Set<string>([taskId]);
  let parent = tasks[taskId]?.parent ?? null;
  while (parent && tasks[parent] && !seen.has(parent)) {
    seen.add(parent);
    out.unshift(tasks[parent]);
    parent = tasks[parent].parent;
  }
  return out;
}

function isLive(task: Task): boolean {
  return task.status.type === "active" || task.status.type === "verifying";
}

/** The rows the tree shows, depth first, with the folding rules applied. */
export function visibleRows(input: TreeInput): TaskTreeView {
  const { tasks, plan, expanded, showReplaced, review } = input;
  const children = childrenOf(tasks);
  const hidden = (t: Task) => t.status.type === "superseded" && !showReplaced;
  const replaced = Object.values(tasks).filter((t) => t.status.type === "superseded").length;

  // Whether a subtree holds something pinned or live, computed once per task.
  const below = new Map<string, { pinned: boolean; live: boolean }>();
  const scan = (task: Task, seen: Set<string>): { pinned: boolean; live: boolean } => {
    const known = below.get(task.id);
    if (known) return known;
    const result = { pinned: false, live: false };
    seen.add(task.id);
    for (const child of children.get(task.id) ?? []) {
      if (seen.has(child.id) || hidden(child)) continue;
      const sub = scan(child, seen);
      result.pinned ||= isPinned(child) || sub.pinned;
      result.live ||= isLive(child) || sub.live;
    }
    below.set(task.id, result);
    return result;
  };

  const rows: TreeRow[] = [];
  const walk = (parentId: string | null, depth: number, seen: Set<string>) => {
    const siblings = (children.get(parentId) ?? []).filter((t) => !hidden(t) && !seen.has(t.id));
    siblings.forEach((task, i) => {
      const kids = (children.get(task.id) ?? []).filter((t) => !hidden(t));
      const sub = scan(task, new Set(seen));
      const open = sub.pinned || (expanded[task.id] ?? (review || sub.live));
      rows.push({
        task,
        depth,
        parentId,
        pos: i + 1,
        size: siblings.length,
        hasChildren: kids.length > 0,
        expanded: kids.length > 0 && open,
        canCollapse: !sub.pinned,
        look: taskLook(task, tasks, plan),
        deps: dependencyBadge(task, tasks),
        progress: kids.length > 0 ? planProgress(kids) : null,
      });
      if (kids.length > 0 && open) walk(task.id, depth + 1, new Set([...seen, task.id]));
    });
  };
  walk(null, 0, new Set());
  return { rows, replaced };
}

/** What a key does in the tree (WAI-ARIA tree pattern). */
export type TreeKeyAction =
  | { type: "focus"; index: number }
  | { type: "expand"; id: string; expanded: boolean }
  | { type: "open"; id: string };

export function treeKeyAction(rows: readonly TreeRow[], index: number, key: string): TreeKeyAction | null {
  const row = rows[index];
  if (!row) return rows.length > 0 && (key === "ArrowDown" || key === "Home") ? { type: "focus", index: 0 } : null;
  switch (key) {
    case "ArrowDown":
      return index < rows.length - 1 ? { type: "focus", index: index + 1 } : null;
    case "ArrowUp":
      return index > 0 ? { type: "focus", index: index - 1 } : null;
    case "Home":
      return { type: "focus", index: 0 };
    case "End":
      return { type: "focus", index: rows.length - 1 };
    case "ArrowRight":
      if (!row.hasChildren) return null;
      return row.expanded ? { type: "focus", index: index + 1 } : { type: "expand", id: row.task.id, expanded: true };
    case "ArrowLeft": {
      if (row.hasChildren && row.expanded && row.canCollapse) return { type: "expand", id: row.task.id, expanded: false };
      const parent = rows.findIndex((r) => r.task.id === row.parentId);
      return parent >= 0 ? { type: "focus", index: parent } : null;
    }
    case "Enter":
      return { type: "open", id: row.task.id };
    default:
      return null;
  }
}
