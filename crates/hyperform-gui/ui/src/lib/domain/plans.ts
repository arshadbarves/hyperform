import type { Message } from "../protocol/Message";
import type { PendingPlan } from "../protocol/PendingPlan";
import type { TodoItem } from "../protocol/TodoItem";
import { MAIN_AGENT, type SessionView } from "./sessionView/types";
import { str } from "./tools/toolInput";

/** What the Plan tab shows: a plan waiting for review, else the last one proposed, else nothing. */
export type PanelPlan =
  | { kind: "pending"; pending: PendingPlan; todos: TodoItem[] }
  | { kind: "last"; plan: string; todos: TodoItem[] }
  | null;

/** The plan text of the newest `ExitPlanMode` call in the transcript; null when there was none. */
export function latestPlan(messages: readonly Message[]): string | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    const message = messages[i];
    if (message?.role !== "assistant") continue;
    for (let j = message.content.length - 1; j >= 0; j--) {
      const block = message.content[j];
      if (block?.type !== "toolUse" || block.name !== "ExitPlanMode") continue;
      const plan = str(block.input, "plan").trim();
      if (plan) return plan;
    }
  }
  return null;
}

/** The main agent's pending plan first, then any helper's; else the chat's last plan with its checklist. */
export function panelPlan(view: Pick<SessionView, "plans" | "messages" | "todos"> | null): PanelPlan {
  if (!view) return null;
  const pending = Object.values(view.plans);
  const first = pending.find((p) => p.agentId === MAIN_AGENT) ?? pending[0];
  if (first) return { kind: "pending", pending: first, todos: view.todos[first.agentId] ?? [] };
  const plan = latestPlan(view.messages);
  return plan ? { kind: "last", plan, todos: view.todos[MAIN_AGENT] ?? [] } : null;
}
