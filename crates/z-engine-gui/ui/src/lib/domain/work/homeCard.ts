import type { PlanSummary } from "../../protocol/PlanSummary";
import type { TaskTone } from "./status";
import { isContinuable, liveChat, needsLine, summaryProgress, summaryStatus } from "./summary";

/** What the agent is told when a plan continues in a new chat. */
export const CONTINUE_PROMPT = "Continue the plan.";

/** The project home's plan card. */
export interface ContinuePlanCard {
  plan: PlanSummary;
  status: string;
  tone: TaskTone;
  progress: string;
  /** 0 to 1, for the bar. */
  fraction: number;
  needs: string | null;
}

/** The most recent plan that is building, paused or waiting on a plan change; null when none is. */
export function continuePlan(plans: readonly PlanSummary[]): ContinuePlanCard | null {
  const plan = plans.filter((p) => isContinuable(p.status)).sort((a, b) => b.updatedAt - a.updatedAt)[0];
  if (!plan) return null;
  const { label, tone } = summaryStatus(plan.status);
  return {
    plan,
    status: label,
    tone,
    progress: summaryProgress(plan),
    fraction: plan.tasksTotal ? Math.min(1, plan.tasksDone / plan.tasksTotal) : 0,
    needs: needsLine(plan.needsYou),
  };
}

/**
 * Where Continue goes: the chat the plan was last in, if it still exists;
 * otherwise a new chat that takes the plan and is told to continue it. A
 * paused plan is not told to continue, since nothing runs until you resume
 * it: the new chat opens on its Plan tab instead.
 */
export type ContinueTarget = { kind: "open"; sessionId: string } | { kind: "new"; prompt: string | null };

export function continueTarget(plan: PlanSummary, chats: readonly { sessionId: string }[]): ContinueTarget {
  const sessionId = liveChat(plan, chats);
  if (sessionId) return { kind: "open", sessionId };
  return { kind: "new", prompt: plan.status === "paused" ? null : CONTINUE_PROMPT };
}
