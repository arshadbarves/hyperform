import { awaitsApply } from "../../domain/work/interrupted";
import type { AgentInfo } from "../../protocol/AgentInfo";
import type { Task } from "../../protocol/Task";
import type { WorkSnapshot } from "../../protocol/WorkSnapshot";

/**
 * Apply and Discard on a helper's worktree in the mock backend, settled
 * roughly as the engine does: a task's helper's change goes to its task,
 * Apply is refused unless that task waits for it (so also once this chat
 * lost the plan), and discarding work kept for an interrupted task leaves
 * the task's resume pointer without its worktree.
 */

export type Decision = "applied" | "discarded";

export interface Settled {
  info: AgentInfo;
  /** The patch for the helper's plan task; empty when nothing on the plan changes. */
  tasks: Map<string, Partial<Task>>;
}

/** The new helper info and task patch, or the engine's refusal notice. */
export function settleHelper(agent: AgentInfo, work: WorkSnapshot | null, decision: Decision): Settled | string {
  const worktree = agent.worktree;
  if (worktree?.state !== "pending" && worktree?.state !== "conflicted") {
    return `Could not ${decision === "applied" ? "apply" : "discard"} agent ${agent.agentId}'s changes: nothing waits in its worktree`;
  }
  const task = agent.taskId ? work?.tasks.find((t) => t.id === agent.taskId) : undefined;
  const waits = task ? awaitsApply(task, agent.agentId) : false;
  if (decision === "applied" && agent.taskId && !waits) {
    return `Could not apply agent ${agent.agentId}'s changes: This helper's work belongs to task ${agent.taskId}. Resume the task or discard its work.`;
  }
  const patch = task ? taskPatch(task, agent.agentId, decision, waits) : null;
  return {
    info: { ...agent, worktree: { ...worktree, state: decision } },
    tasks: task && patch ? new Map([[task.id, patch]]) : new Map(),
  };
}

function taskPatch(task: Task, agentId: string, decision: Decision, waits: boolean): Partial<Task> | null {
  if (!waits) return task.resume?.agentId === agentId ? { resume: { ...task.resume, worktree: null } } : null;
  if (decision === "applied") return { status: { type: "done" }, run: { ...task.run, integration: { type: "applied", files: [] } } };
  return { status: { type: "blocked", reason: { type: "changeDiscarded" } }, run: { ...task.run, integration: { type: "discarded" } } };
}
