import type { PlanSummary } from "../../protocol/PlanSummary";
import type { NeedsYouItem } from "../inbox";
import { wsBasename } from "../paths";
import { liveChat, needsLine } from "./summary";

/**
 * One Needs you item per work plan that waits on you: the goal and how
 * many things need you. The plan's own Needs you strip lists them; the
 * Inbox does not repeat them per task. Under the chat it was last in, or
 * the project when that chat is gone.
 */
export function planInboxItems(
  plans: readonly PlanSummary[],
  chats: readonly { sessionId: string }[],
  titleOf: (sessionId: string) => string,
): NeedsYouItem[] {
  return plans
    .filter((p) => p.needsYou > 0)
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .map((plan) => {
      const chat = liveChat(plan, chats);
      return {
        key: `plan:${plan.planId}`,
        sessionId: chat ?? "",
        chatTitle: chat ? titleOf(chat) : wsBasename(plan.projectRoot),
        kind: "workPlan",
        title: plan.goal,
        detail: needsLine(plan.needsYou),
        requestId: null,
        urgency: null,
        plan,
      };
    });
}
