import { describe, expect, it } from "vitest";
import { finishedChats, inboxCount, inboxNotices, needsYou } from "./inbox";
import { emptyView, type SessionView } from "./sessionView/types";
import { approval } from "./testFixtures";

const title = (id: string) => `Chat ${id}`;

function view(id: string, over: Partial<SessionView> = {}): SessionView {
  return { ...emptyView(id), ...over };
}

describe("needsYou", () => {
  it("collects approvals, questions, plans and trust across chats, chat by chat", () => {
    const views = {
      a: view("a", { approvals: { r1: approval({ requestId: "r1", title: "Run cargo test" }) } }),
      b: view("b", {
        questions: { q1: { requestId: "q1", agentId: "main", questions: [{ question: "Which db?", header: "", options: [], multiSelect: false }] } },
        plans: { p1: { requestId: "p1", agentId: "main", plan: "# Plan" } },
      }),
      c: view("c", { trustRequest: { projectRoot: "/p", defines: ["hooks"] } }),
      d: view("d"),
    };
    const items = needsYou(views, title);
    expect(items.map((i) => [i.sessionId, i.kind, i.title])).toEqual([
      ["a", "approval", "Run cargo test"],
      ["b", "question", "Which db?"],
      ["b", "plan", "Plan ready for review"],
      ["c", "trust", "Trust this project?"],
    ]);
    expect(items[0]).toMatchObject({ chatTitle: "Chat a", requestId: "r1" });
  });
});

describe("finishedChats", () => {
  it("lists background chats whose turn ended, newest first, skipping busy ones", () => {
    const unread = {
      a: { outcome: { type: "completed" as const }, verification: { status: "verified" as const, checks: [] }, at: 5 },
      b: { outcome: { type: "failed" as const, message: "boom" }, verification: { status: "notApplicable" as const }, at: 9 },
      c: { outcome: { type: "completed" as const }, verification: { status: "notApplicable" as const }, at: 7 },
    };
    const items = finishedChats(unread, { c: "working" }, title);
    expect(items.map((i) => [i.sessionId, i.label, i.tone])).toEqual([
      ["b", "Response failed", "danger"],
      ["a", "Finished · verified", "ok"],
    ]);
  });
});

describe("inboxNotices", () => {
  it("merges passing notices with every chat's warnings and errors, newest first", () => {
    const views = {
      a: view("a", {
        notices: [{ id: 1, level: "warn", text: "Checkpoints are off", at: 20 }],
        errors: [{ id: 2, message: "Provider said no", afterMessageId: null, at: 30 }],
      }),
    };
    const toasts = [{ key: "t1", tone: "info" as const, title: "Copied", text: "Copied", sessionId: null, at: 25 }];
    const list = inboxNotices(toasts, views, title);
    expect(list.map((n) => [n.key, n.tone, n.chatTitle])).toEqual([
      ["a:e2", "error", "Chat a"],
      ["t1", "info", null],
      ["a:n1", "warn", "Chat a"],
    ]);
  });
});

describe("inboxCount", () => {
  it("counts what needs you, unread results, and unread problems", () => {
    const notices = [
      { key: "1", tone: "error" as const, title: "x", text: "x", sessionId: null, chatTitle: null, at: 10, urgency: null },
      { key: "2", tone: "info" as const, title: "y", text: "y", sessionId: null, chatTitle: null, at: 11, urgency: null },
      { key: "3", tone: "warn" as const, title: "z", text: "z", sessionId: null, chatTitle: null, at: 2, urgency: null },
    ];
    expect(inboxCount({ needsYou: 2, finished: 1, notices, readAt: 5 })).toBe(4);
  });
});
