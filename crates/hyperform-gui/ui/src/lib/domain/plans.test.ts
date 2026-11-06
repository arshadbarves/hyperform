import { describe, expect, it } from "vitest";
import type { TodoItem } from "../protocol/TodoItem";
import { latestPlan, panelPlan } from "./plans";
import { emptyView } from "./sessionView/types";
import { assistant, text, toolUse, user } from "./testFixtures";

const todo = (content: string): TodoItem => ({ content, activeForm: content, status: "pending" });

describe("latestPlan", () => {
  it("finds the newest ExitPlanMode plan", () => {
    const messages = [
      user("u1", "Plan it"),
      assistant("a1", [text("First"), toolUse("c1", "ExitPlanMode", { plan: "Old plan" })]),
      user("u2", "Again"),
      assistant("a2", [toolUse("c2", "Read", { file_path: "a.rs" }), toolUse("c3", "ExitPlanMode", { plan: "New plan" })]),
      assistant("a3", "Done"),
    ];
    expect(latestPlan(messages)).toBe("New plan");
  });

  it("skips empty plans and other tools", () => {
    const messages = [assistant("a1", [toolUse("c1", "ExitPlanMode", { plan: "Real" })]), assistant("a2", [toolUse("c2", "ExitPlanMode", { plan: "  " })])];
    expect(latestPlan(messages)).toBe("Real");
    expect(latestPlan([assistant("a1", [toolUse("c1", "Edit", { plan: "no" })])])).toBeNull();
  });
});

describe("panelPlan", () => {
  it("prefers a pending plan, the main agent's first, with its checklist", () => {
    const view = emptyView("s1");
    view.plans = {
      r2: { requestId: "r2", agentId: "agt_1", plan: "Helper plan" },
      r1: { requestId: "r1", agentId: "main", plan: "Main plan" },
    };
    view.todos = { main: [todo("Write tests")] };
    view.messages = [assistant("a1", [toolUse("c1", "ExitPlanMode", { plan: "Older" })])];
    const shown = panelPlan(view);
    expect(shown?.kind).toBe("pending");
    expect(shown?.kind === "pending" && shown.pending.requestId).toBe("r1");
    expect(shown?.todos).toEqual([todo("Write tests")]);
  });

  it("falls back to the chat's last plan", () => {
    const view = emptyView("s1");
    view.messages = [assistant("a1", [toolUse("c1", "ExitPlanMode", { plan: "Shipped plan" })])];
    expect(panelPlan(view)).toEqual({ kind: "last", plan: "Shipped plan", todos: [] });
  });

  it("is null with no plan at all", () => {
    expect(panelPlan(emptyView("s1"))).toBeNull();
    expect(panelPlan(null)).toBeNull();
  });
});
