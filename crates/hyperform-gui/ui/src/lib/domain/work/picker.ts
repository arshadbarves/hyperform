import type { PlanSummary } from "../../protocol/PlanSummary";
import type { TaskTone } from "./status";
import { attachedChat, isSettled, needsLine, summaryProgress, summaryStatus } from "./summary";

/** One plan in the "Continue a plan" list. */
export interface PickerRow {
  plan: PlanSummary;
  status: string;
  tone: TaskTone;
  /** "2 of 5 done". */
  progress: string;
  /** "3 things need you"; null when nothing does. */
  needs: string | null;
  /** Another chat it is attached to now, which still exists: continuing moves it here. Null once detached. */
  elsewhere: string | null;
}

/** Open plans first in the list; done and abandoned ones behind a disclosure. */
export interface PlanPicker {
  open: PickerRow[];
  settled: PickerRow[];
}

export interface PickerContext {
  /** The plan attached to this chat, left out of the list. */
  currentPlanId: string | null;
  /** This chat; a plan attached to it does not move. */
  sessionId: string | null;
  /** The project's chats that still exist. */
  chats: readonly { sessionId: string }[];
}

function row(plan: PlanSummary, ctx: PickerContext): PickerRow {
  const { label, tone } = summaryStatus(plan.status);
  const chat = attachedChat(plan, ctx.chats);
  return {
    plan,
    status: label,
    tone,
    progress: summaryProgress(plan),
    needs: needsLine(plan.needsYou),
    elsewhere: chat && chat !== ctx.sessionId ? chat : null,
  };
}

/** The project's plans to continue here, newest first. */
export function planPicker(plans: readonly PlanSummary[], ctx: PickerContext): PlanPicker {
  const rows = plans
    .filter((p) => p.planId !== ctx.currentPlanId)
    .slice()
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .map((p) => row(p, ctx));
  return {
    open: rows.filter((r) => !isSettled(r.plan.status)),
    settled: rows.filter((r) => isSettled(r.plan.status)),
  };
}

/** Plans this chat could continue: any but the one attached. */
export function canPickPlan(plans: readonly PlanSummary[], currentPlanId: string | null): boolean {
  return plans.some((p) => p.planId !== currentPlanId);
}

/** The confirm before continuing a plan another chat has: it moves here. */
export function moveNotice(goal: string, chatTitle: string): { title: string; description: string } {
  return {
    title: "Move this plan to this chat?",
    description: `“${goal}” is in “${chatTitle}”. Continuing it here moves it to this chat; that chat keeps its messages but no longer has the plan.`,
  };
}
