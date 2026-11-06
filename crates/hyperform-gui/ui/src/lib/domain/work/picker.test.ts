import { describe, expect, it } from "vitest";
import { summary } from "./fixtures";
import { canPickPlan, moveNotice, planPicker } from "./picker";
import { attachedChat, isContinuable, isSettled, liveChat, needsLine, summaryProgress, summaryStatus } from "./summary";

const chats = [{ sessionId: "S1" }, { sessionId: "S2" }];

describe("plan summaries", () => {
  it("words a listed plan as its Plan tab does", () => {
    expect(summaryStatus("active")).toEqual({ label: "Building", tone: "working" });
    expect(summaryStatus("replanning").tone).toBe("attention");
    expect(summaryProgress(summary("P1", { tasksDone: 2, tasksTotal: 5 }))).toBe("2 of 5 done");
    expect([needsLine(0), needsLine(1), needsLine(3)]).toEqual([null, "1 thing needs you", "3 things need you"]);
  });

  it("knows what continues and what is settled", () => {
    expect((["draft", "active", "replanning", "paused", "done", "abandoned"] as const).map(isContinuable)).toEqual([
      false,
      true,
      true,
      true,
      false,
      false,
    ]);
    expect(isSettled("done") && isSettled("abandoned") && !isSettled("paused")).toBe(true);
  });

  it("finds the chat a plan was last in only while it exists", () => {
    expect(liveChat(summary("P1", { lastSessionId: "S2" }), chats)).toBe("S2");
    expect(liveChat(summary("P1", { lastSessionId: "gone" }), chats)).toBeNull();
    expect(liveChat(summary("P1"), chats)).toBeNull();
  });

  it("finds the chat a plan is attached to now, not the one it was last in", () => {
    expect(attachedChat(summary("P1", { lastSessionId: "S2", attachedTo: "S2" }), chats)).toBe("S2");
    expect(attachedChat(summary("P1", { lastSessionId: "S2", attachedTo: null }), chats)).toBeNull();
    expect(attachedChat(summary("P1", { attachedTo: "gone" }), chats)).toBeNull();
  });
});

describe("plan picker", () => {
  const plans = [
    summary("old", { updatedAt: 100, status: "paused" }),
    summary("here", { updatedAt: 500, lastSessionId: "S1", attachedTo: "S1" }),
    summary("new", { updatedAt: 900, status: "replanning", needsYou: 2, lastSessionId: "S2", attachedTo: "S2" }),
    summary("done", { updatedAt: 800, status: "done", tasksDone: 5 }),
    summary("gone", { updatedAt: 300, status: "abandoned", lastSessionId: "deleted", attachedTo: "deleted" }),
    summary("left", { updatedAt: 400, status: "paused", lastSessionId: "S2", attachedTo: null }),
    summary("mine", { updatedAt: 950 }),
  ];

  it("lists open plans newest first, leaves the attached one out and settles done and abandoned ones apart", () => {
    const picker = planPicker(plans, { currentPlanId: "mine", sessionId: "S1", chats });
    expect(picker.open.map((r) => r.plan.planId)).toEqual(["new", "here", "left", "old"]);
    expect(picker.settled.map((r) => r.plan.planId)).toEqual(["done", "gone"]);
  });

  it("says a plan moves only when another chat has it attached now", () => {
    const picker = planPicker(plans, { currentPlanId: null, sessionId: "S1", chats });
    const by = Object.fromEntries([...picker.open, ...picker.settled].map((r) => [r.plan.planId, r]));
    expect(by.new).toMatchObject({ status: "Replanning", tone: "attention", needs: "2 things need you", elsewhere: "S2" });
    expect(by.here.elsewhere).toBeNull();
    expect(by.gone.elsewhere).toBeNull();
    expect(by.left.elsewhere).toBeNull();
    expect(moveNotice("Ship it", "Sync work").description).toContain("moves it to this chat");
  });

  it("offers the picker only when another plan exists", () => {
    expect(canPickPlan([summary("mine")], "mine")).toBe(false);
    expect(canPickPlan(plans, "mine")).toBe(true);
    expect(canPickPlan([], null)).toBe(false);
  });
});
