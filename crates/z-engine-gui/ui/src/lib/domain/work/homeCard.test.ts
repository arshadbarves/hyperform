import { describe, expect, it } from "vitest";
import { summary } from "./fixtures";
import { CONTINUE_PROMPT, continuePlan, continueTarget } from "./homeCard";

describe("home plan card", () => {
  it("shows the most recent plan that is building, paused or replanning", () => {
    const card = continuePlan([
      summary("draft", { status: "draft", updatedAt: 999 }),
      summary("done", { status: "done", updatedAt: 998 }),
      summary("paused", { status: "paused", updatedAt: 500 }),
      summary("active", { status: "active", updatedAt: 700, tasksDone: 3, tasksTotal: 4, needsYou: 1 }),
    ]);
    expect(card).toMatchObject({
      status: "Building",
      tone: "working",
      progress: "3 of 4 done",
      fraction: 0.75,
      needs: "1 thing needs you",
    });
    expect(card?.plan.planId).toBe("active");
  });

  it("is null without such a plan, so the chats card stays as it is", () => {
    expect(continuePlan([summary("d", { status: "done" }), summary("a", { status: "abandoned" })])).toBeNull();
    expect(continuePlan([])).toBeNull();
    expect(continuePlan([summary("e", { tasksTotal: 0, tasksDone: 0 })])?.fraction).toBe(0);
  });

  it("continues in the chat the plan was last in, else in a new chat told to continue it", () => {
    const chats = [{ sessionId: "S1" }];
    expect(continueTarget(summary("p", { lastSessionId: "S1" }), chats)).toEqual({ kind: "open", sessionId: "S1" });
    expect(continueTarget(summary("p", { lastSessionId: "S1", attachedTo: null }), chats)).toEqual({ kind: "open", sessionId: "S1" });
    expect(continueTarget(summary("p", { lastSessionId: "gone" }), chats)).toEqual({ kind: "new", prompt: CONTINUE_PROMPT });
    expect(continueTarget(summary("p", { status: "paused" }), chats)).toEqual({ kind: "new", prompt: null });
    expect(CONTINUE_PROMPT).toBe("Continue the plan.");
  });
});
