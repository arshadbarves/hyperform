import { describe, expect, it } from "vitest";
import { byUrgency, inboxNotices, needsYou } from "./inbox";
import { emptyView, type SessionView } from "./sessionView/types";
import { approval } from "./testFixtures";

const title = (id: string) => `Chat ${id}`;

function view(id: string, over: Partial<SessionView> = {}): SessionView {
  return { ...emptyView(id), ...over };
}

const notice = (id: number, text: string, at: number) => ({ id, level: "warn" as const, text, at });

describe("inbox urgency (decisions_inbox_priority)", () => {
  it("sorts high first, unrated as normal, low last, keeping the order of ties", () => {
    const items = [
      { id: 1, urgency: null },
      { id: 2, urgency: "low" as const },
      { id: 3, urgency: "high" as const },
      { id: 4, urgency: "normal" as const },
      { id: 5, urgency: null },
    ];
    expect(byUrgency(items).map((i) => i.id)).toEqual([3, 1, 4, 5, 2]);
  });

  it("orders needs-you items by their request's urgency", () => {
    const approvals = {
      r1: approval({ requestId: "r1", title: "Run ls" }),
      r2: approval({ requestId: "r2", title: "Delete build" }),
    };
    const views = {
      a: view("a", { approvals, urgency: { r1: "low", r2: "high" } }),
      b: view("b", { trustRequest: { projectRoot: "/p", defines: [] } }),
    };
    const items = needsYou(views, title);
    expect(items.map((i) => [i.title, i.urgency])).toEqual([
      ["Delete build", "high"],
      ["Trust this project?", null],
      ["Run ls", "low"],
    ]);
  });

  it("keeps today's order when nothing was rated", () => {
    const views = { a: view("a", { approvals: { r1: approval({ requestId: "r1" }), r2: approval({ requestId: "r2" }) } }) };
    expect(needsYou(views, title).map((i) => i.requestId)).toEqual(["r1", "r2"]);
    const plain = { a: view("a", { notices: [notice(1, "old", 1), notice(2, "new", 2)] }) };
    expect(inboxNotices([], plain, title).map((n) => n.text)).toEqual(["new", "old"]);
  });

  it("puts urgent notices first, matched by their text, then newest first", () => {
    const views = {
      a: view("a", {
        notices: [notice(1, "disk full", 1), notice(2, "routine", 3), notice(3, "slow hook", 2)],
        urgency: { "notice:disk full": "high", "notice:routine": "low" },
      }),
    };
    const recorded = [{ key: "x", tone: "info" as const, title: "Saved", text: "Saved", sessionId: null, at: 4 }];
    const notices = inboxNotices(recorded, views, title);
    expect(notices.map((n) => [n.text, n.urgency])).toEqual([
      ["disk full", "high"],
      ["Saved", null],
      ["slow hook", null],
      ["routine", "low"],
    ]);
  });
});
