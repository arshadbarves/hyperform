import type { AgentInfo } from "../../protocol/AgentInfo";
import type { PlanStatus } from "../../protocol/PlanStatus";
import type { Task } from "../../protocol/Task";
import type { WorkView } from "../sessionView/work";

/**
 * The task waits for this helper's change, as the engine's `awaits`
 * (work/helper.rs) decides: its current run is the helper's and the change
 * has not come back (Active, perhaps parked at its merge card), or it came
 * back conflicted and the kept worktree is still to apply or discard.
 */
export function awaitsApply(task: Task, agentId: string): boolean {
  if (task.run.dispatch?.agentId !== agentId) return false;
  const integration = task.run.integration;
  if (!integration || integration.type === "pendingApply") return task.status.type === "active";
  if (integration.type === "conflict") return task.status.type === "blocked" && task.status.reason.type === "mergeConflict";
  return false;
}

/** A task-bound helper the plan interrupted: its task picks up from its work, which only Discard throws away. */
export interface InterruptedHelper {
  task: Task;
  /** "Interrupted · kept for task “Convert old settings files”". */
  line: string;
  /** Where it stopped, in the engine's words; null when it left none. */
  note: string | null;
  /** How the kept work comes back into play. */
  hint: string;
}

const REOPEN = "Reopen the plan to pick the task up from this work.";
const HINT: Partial<Record<PlanStatus, string>> = {
  paused: "Resume the plan and the task picks up from this work.",
  done: REOPEN,
  abandoned: REOPEN,
};
const RUNS_AGAIN = "The task picks up from this work when it runs again.";

/**
 * The helper's work is kept for its task: it was sent for a plan task
 * (`taskId`) whose resume pointer names it, and that task does not wait on
 * its apply (the engine refuses Apply then). Null otherwise.
 */
export function interruptedHelper(agent: AgentInfo, work: WorkView | null): InterruptedHelper | null {
  const task = agent.taskId && work ? work.tasks[agent.taskId] : undefined;
  const pointer = task?.resume;
  if (!work || !task || !pointer || pointer.agentId !== agent.agentId || awaitsApply(task, agent.agentId)) return null;
  return {
    task,
    line: `Interrupted · kept for task “${task.title}”`,
    note: pointer.note.trim() || null,
    hint: HINT[work.plan.status] ?? RUNS_AGAIN,
  };
}
