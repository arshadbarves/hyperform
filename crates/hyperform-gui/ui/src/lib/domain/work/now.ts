import type { AgentInfo } from "../../protocol/AgentInfo";
import type { Task } from "../../protocol/Task";
import { MAIN_AGENT } from "../sessionView/types";
import type { WorkView } from "../sessionView/work";
import { criteriaMet, stepLabel, stepProgress, type Progress } from "./progress";

/** Who works a running task: this chat's main agent, one of its helpers, another chat, or nobody (verifying). */
export type LaneOwner = "main" | "helper" | "other" | "none";

/** One running task in the Plan tab's Now: the main agent's, then one lane per helper. */
export interface NowLane {
  task: Task;
  owner: LaneOwner;
  /** The helper in this chat, for its status and the link to its row in the Agents tab. */
  agent: AgentInfo | null;
  agentId: string | null;
  /** The helper's own worktree, when it has one. */
  worktree: string | null;
  step: string;
  steps: Progress;
  criteria: Progress;
}

const OWNER_ORDER: Record<LaneOwner, number> = { main: 0, helper: 1, other: 2, none: 3 };

function laneOf(task: Task, sessionId: string | null, agents: Readonly<Record<string, AgentInfo>>): NowLane {
  const sent = task.run.dispatch;
  const owner = task.owner ?? (sent ? { sessionId: sent.sessionId, agentId: sent.agentId } : null);
  const here = owner !== null && owner.sessionId === sessionId;
  const kind: LaneOwner = !owner ? "none" : !here ? "other" : owner.agentId === MAIN_AGENT ? "main" : "helper";
  const agentId = kind === "helper" && owner ? owner.agentId : null;
  return {
    task,
    owner: kind,
    agent: agentId ? (agents[agentId] ?? null) : null,
    agentId,
    worktree: kind === "helper" ? (task.run.dispatch?.worktree ?? null) : null,
    step: stepLabel(task),
    steps: stepProgress(task),
    criteria: criteriaMet(task),
  };
}

/** Active and verifying tasks: the main agent's first, then helpers by when they were sent, then the rest. */
export function nowLanes(
  work: WorkView | null,
  sessionId: string | null,
  agents: Readonly<Record<string, AgentInfo>>,
): NowLane[] {
  if (!work) return [];
  const running = Object.values(work.tasks).filter((t) => t.status.type === "active" || t.status.type === "verifying");
  return running
    .map((task) => laneOf(task, sessionId, agents))
    .sort(
      (a, b) =>
        OWNER_ORDER[a.owner] - OWNER_ORDER[b.owner] ||
        (a.task.run.dispatch?.at ?? a.task.updatedAt) - (b.task.run.dispatch?.at ?? b.task.updatedAt),
    );
}

/**
 * The lane's progress line, "3 of 5 steps · Running the sync tests": its
 * step in progress, else what its helper is doing now (`activity`), else
 * the task's own active form; never the task's title again. Null when
 * there is nothing to say.
 */
export function laneStep(lane: NowLane, activity: string | null = null): string | null {
  const stepping = lane.steps.done < lane.steps.total;
  const own = lane.step !== lane.task.title ? lane.step : null;
  const step = stepping ? lane.step : (activity ?? own);
  const count = lane.steps.total ? `${lane.steps.done} of ${lane.steps.total} steps` : null;
  return [count, step].filter(Boolean).join(" · ") || null;
}

/**
 * While the plan is paused, what happens to a lane's work: helpers stop at
 * their next step and keep their worktree, the main agent finishes its
 * current step, a running check still finishes.
 */
export function laneStopLine(lane: NowLane): string {
  switch (lane.owner) {
    case "main":
      return "Stops after its current step";
    case "other":
      return "Stopping in another chat";
    case "none":
      return "Its checks still finish";
    default: {
      const kept = lane.worktree ? " · its worktree is kept" : "";
      const live = !lane.agent || lane.agent.status === "running" || lane.agent.status === "waiting";
      return live ? `Stopping at its next step${kept}` : `Stopped${kept}`;
    }
  }
}

/** "Criteria 1 of 3 met", or null for a task without criteria. */
export function laneCriteria(lane: NowLane): string | null {
  return lane.criteria.total ? `Criteria ${lane.criteria.done} of ${lane.criteria.total} met` : null;
}

/** The plan task a helper works on: its `taskId`, else the task dispatched to it in this chat. */
export function helperTask(agent: AgentInfo, work: WorkView | null, sessionId: string | null): Task | null {
  if (!work) return null;
  const bound = agent.taskId ? work.tasks[agent.taskId] : undefined;
  if (bound) return bound;
  return (
    Object.values(work.tasks).find(
      (t) => t.run.dispatch?.agentId === agent.agentId && t.run.dispatch.sessionId === sessionId,
    ) ?? null
  );
}
