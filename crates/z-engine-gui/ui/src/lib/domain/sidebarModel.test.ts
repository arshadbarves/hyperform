import { describe, expect, it } from "vitest";
import type { SessionListItem } from "./sessionList";
import { CHATS_SHOWN, sidebarModel, turnsByProject, type SidebarInput } from "./sidebarModel";
import { emptyView } from "./sessionView/types";
import { turnRecord } from "./testFixtures";

const NOW = 10 * 3_600_000;

function chat(id: string, root: string, over: Partial<SessionListItem> = {}): SessionListItem {
  return { sessionId: id, title: `Chat ${id}`, projectRoot: root, updatedAt: NOW - 120_000, legacy: false, costUsd: 0, lastOutcome: null, ...over };
}

function input(over: Partial<SidebarInput> = {}): SidebarInput {
  return {
    sessions: [],
    roots: ["/work/app", "/work/api"],
    activeRoot: "/work/app",
    activeSessionId: null,
    activity: {},
    unread: {},
    expanded: {},
    now: NOW,
    ...over,
  };
}

describe("sidebarModel", () => {
  it("groups chats under their project and keeps the rest apart", () => {
    const model = sidebarModel(
      input({ sessions: [chat("a", "/work/app/"), chat("b", "/work/api"), chat("c", "/elsewhere")] }),
    );
    expect(model.projects.map((p) => [p.name, p.active, p.rows.map((r) => r.item.sessionId)])).toEqual([
      ["app", true, ["a"]],
      ["api", false, ["b"]],
    ]);
    expect(model.others.map((r) => r.item.sessionId)).toEqual(["c"]);
  });

  it("shows the first chats of a project and counts the rest until expanded", () => {
    const many = Array.from({ length: CHATS_SHOWN + 3 }, (_, i) => chat(`c${i}`, "/work/app"));
    const folded = sidebarModel(input({ sessions: many })).projects[0];
    expect(folded.rows).toHaveLength(CHATS_SHOWN);
    expect(folded.hidden).toBe(3);
    const open = sidebarModel(input({ sessions: many, expanded: { "/work/app": true } })).projects[0];
    expect(open.rows).toHaveLength(CHATS_SHOWN + 3);
    expect(open.hidden).toBe(0);
  });

  it("never hides the open chat behind Show more", () => {
    const many = Array.from({ length: CHATS_SHOWN + 3 }, (_, i) => chat(`c${i}`, "/work/app"));
    const group = sidebarModel(input({ sessions: many, activeSessionId: "c10" })).projects[0];
    expect(group.rows.map((r) => r.item.sessionId)).toContain("c10");
    expect(group.rows).toHaveLength(CHATS_SHOWN + 1);
    expect(group.hidden).toBe(2);
  });

  it("labels rows with their age and one mark; the open chat has none", () => {
    const model = sidebarModel(
      input({
        sessions: [chat("a", "/work/app"), chat("b", "/work/app", { updatedAt: NOW - 3 * 3_600_000 })],
        activeSessionId: "a",
        activity: { a: "working", b: "approval" },
      }),
    );
    const [a, b] = model.projects[0].rows;
    expect([a.when, a.active, a.mark]).toEqual(["2m", true, null]);
    expect([b.when, b.mark?.tone]).toEqual(["3h", "attention"]);
  });

  it("lets a project carry its most urgent live mark for when it is folded", () => {
    const sessions = [chat("a", "/work/api"), chat("b", "/work/api")];
    const working = sidebarModel(input({ sessions, activity: { a: "working" } })).projects[1];
    expect(working.mark?.tone).toBe("working");
    const waiting = sidebarModel(input({ sessions, activity: { a: "working", b: "approval" } })).projects[1];
    expect(waiting.mark).toEqual({ tone: "attention", label: "A chat here needs you" });
  });
});

describe("turnsByProject", () => {
  it("sums finished turns per project of the live chats", () => {
    const view = (id: string, root: string, turns: number) => ({
      ...emptyView(id),
      info: { sessionId: id, title: null, projectRoot: root, model: "m", mode: "default" as const, planning: false, effort: null, createdAt: 0, updatedAt: 0, legacy: false },
      turns: Array.from({ length: turns }, (_, i) => turnRecord({ turnId: `${id}${i}` })),
    });
    expect(turnsByProject({ a: view("a", "/w/app", 2), b: view("b", "/w/app", 1), c: view("c", "/w/api", 0) })).toEqual({
      "/w/app": 3,
      "/w/api": 0,
    });
  });
});
