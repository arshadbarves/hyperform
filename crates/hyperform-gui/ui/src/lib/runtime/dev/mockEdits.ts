import type { PlanStatus } from "../../protocol/PlanStatus";
import type { Task } from "../../protocol/Task";
import type { WorkEdit } from "../../protocol/WorkEdit";
import type { WorkSnapshot } from "../../protocol/WorkSnapshot";
import { healthOf } from "./mockPlans";

/**
 * A rough stand-in for the work kernel in the mock backend: the plan
 * lifecycle edits (with the kernel's allowed moves), hold and release, Stop
 * releasing bound tasks, and a few task edits. Anything else only bumps the
 * revision. Returns the new plan and the tasks it changed, or the refusal.
 */

export interface MockEdit {
  work: WorkSnapshot;
  tasks: Task[];
}

const MOVES: Partial<Record<WorkEdit["type"], [PlanStatus, PlanStatus[]]>> = {
  approve: ["active", ["draft", "replanning"]],
  pause: ["paused", []],
  resume: ["active", ["paused"]],
  abandon: ["abandoned", []],
  reopen: ["active", ["done", "abandoned"]],
  requestReplan: ["replanning", []],
};

/**
 * Tasks bound to a running helper, as Stop or a paused helper's next step
 * leaves them: Pending, unbound, resumable. A task whose helper's change
 * came back (waiting to apply, or conflicted) stays as it is.
 */
export function releaseBound(work: WorkSnapshot, note: string): Map<string, Partial<Task>> {
  const out = new Map<string, Partial<Task>>();
  for (const t of work.tasks) {
    if (!t.owner || t.owner.agentId === "main" || t.run.integration) continue;
    const worktree = t.run.dispatch?.worktree ?? null;
    out.set(t.id, {
      owner: null,
      status: t.status.type === "active" ? { type: "pending" } : t.status,
      resume: { sessionId: t.owner.sessionId, agentId: t.owner.agentId, worktree, note },
    });
  }
  return out;
}

function taskPatch(edit: WorkEdit, work: WorkSnapshot): Map<string, Partial<Task>> | string {
  const one = (taskId: string, patch: Partial<Task>) =>
    work.tasks.some((t) => t.id === taskId) ? new Map([[taskId, patch]]) : `no task ${taskId}`;
  switch (edit.type) {
    case "stop":
      return releaseBound(work, "Stopped by you");
    case "holdTask":
      return one(edit.taskId, { held: edit.held });
    case "releaseTask":
      return one(edit.taskId, { held: false });
    case "acceptTask":
      return one(edit.taskId, { status: { type: "pending" } });
    case "cancelTask":
      return one(edit.taskId, { status: { type: "cancelled" }, owner: null });
    case "retryTask":
      return one(edit.taskId, { status: { type: "pending" }, attempts: 0 });
    case "renameTask":
      return one(edit.taskId, { title: edit.title });
    default:
      return new Map();
  }
}

function nextStatus(edit: WorkEdit, from: PlanStatus): { to: PlanStatus } | { refused: string } {
  if (edit.type === "stop") {
    return from === "active" || from === "replanning" || from === "paused" ? { to: "paused" } : { refused: `the plan is ${from}` };
  }
  const move = MOVES[edit.type];
  if (!move) return { to: from };
  const [to, allowed] = move;
  if (allowed.length && !allowed.includes(from)) return { refused: `the plan is ${from} and cannot ${edit.type}` };
  if (to === "abandoned" && from === "done") return { refused: "a done plan cannot be abandoned; reopen it first" };
  return { to };
}

/** Apply `edit`; patches from `extra` (a helper reaching its next step) apply too. */
export function editPlan(work: WorkSnapshot, edit: WorkEdit | null, now: number, extra = new Map<string, Partial<Task>>()): MockEdit | string {
  const move = edit ? nextStatus(edit, work.plan.status) : { to: work.plan.status };
  if ("refused" in move) return move.refused;
  const status = move.to;
  const patches = edit ? taskPatch(edit, work) : new Map<string, Partial<Task>>();
  if (typeof patches === "string") return patches;
  for (const [id, patch] of extra) patches.set(id, { ...patches.get(id), ...patch });
  const revision = work.plan.revision + 1;
  const changed: Task[] = [];
  const tasks = work.tasks.map((t) => {
    const patch = patches.get(t.id);
    if (!patch) return t;
    const next = { ...t, ...patch, revision, updatedAt: now };
    changed.push(next);
    return next;
  });
  const accepted = edit?.type === "approve" && work.plan.status === "replanning";
  const parallel = edit?.type === "approveParallel" ? "approved" : work.plan.parallel;
  return {
    work: {
      ...work,
      plan: { ...work.plan, status, parallel, revision, updatedAt: now },
      tasks,
      health: healthOf(tasks),
      findings: accepted ? [] : work.findings,
      replanHeld: accepted || status !== "replanning" ? [] : work.replanHeld,
    },
    tasks: changed,
  };
}
