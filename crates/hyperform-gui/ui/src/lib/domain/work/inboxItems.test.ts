import { describe, expect, it } from "vitest";
import { summary } from "./fixtures";
import { planInboxItems } from "./inboxItems";

describe("work plans in the Inbox", () => {
  const titleOf = (id: string) => `Chat ${id}`;

  it("adds one item per plan that waits on you, newest first, under its chat", () => {
    const items = planInboxItems(
      [
        summary("quiet", { needsYou: 0 }),
        summary("a", { goal: "Offline sync", needsYou: 3, lastSessionId: "S1", updatedAt: 100 }),
        summary("b", { goal: "Status badge", needsYou: 1, lastSessionId: "gone", updatedAt: 200, projectRoot: "/work/app" }),
      ],
      [{ sessionId: "S1" }],
      titleOf,
    );
    expect(items).toEqual([
      expect.objectContaining({ key: "plan:b", kind: "workPlan", sessionId: "", chatTitle: "app", title: "Status badge", detail: "1 thing needs you" }),
      expect.objectContaining({ key: "plan:a", sessionId: "S1", chatTitle: "Chat S1", title: "Offline sync", detail: "3 things need you" }),
    ]);
    expect(items[1].plan?.planId).toBe("a");
  });

  it("keeps a detached plan under the chat it was last in", () => {
    const [item] = planInboxItems([summary("a", { needsYou: 1, lastSessionId: "S1", attachedTo: null })], [{ sessionId: "S1" }], titleOf);
    expect(item).toMatchObject({ sessionId: "S1", chatTitle: "Chat S1" });
  });

  it("adds nothing while no plan waits on you", () => {
    expect(planInboxItems([summary("a")], [], titleOf)).toEqual([]);
  });
});
