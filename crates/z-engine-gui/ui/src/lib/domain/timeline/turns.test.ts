import { describe, expect, it } from "vitest";
import { assistant, text, thinking, toolResult, toolUse, turnRecord, user } from "../testFixtures";
import { buildTimeline, type TimelineItem } from "./turns";

const kinds = (items: TimelineItem[]) => items.map((i) => i.kind);

describe("buildTimeline", () => {
  it("groups a prompt with its assistant and tool rounds", () => {
    const turns = buildTimeline({
      messages: [
        user("u1", "fix it", 1),
        assistant("a1", [thinking("plan"), text("Looking"), toolUse("c1", "Read", { file_path: "a.rs" })], 2),
        user("r1", [toolResult("c1", "fn main")], 3),
        assistant("a2", "Done", 4),
      ],
      turns: [turnRecord({ messageId: "u1", finishedAt: 10 })],
    });
    expect(turns).toHaveLength(1);
    expect(turns[0].user?.id).toBe("u1");
    expect(turns[0].record?.turnId).toBe("t1");
    expect(kinds(turns[0].items)).toEqual(["thinking", "text", "tool", "text"]);
    expect(turns[0].active).toBe(false);
  });

  it("starts a new turn for each prompt", () => {
    const turns = buildTimeline({
      messages: [user("u1", "one", 1), assistant("a1", "ok", 2), user("u2", "two", 3), assistant("a2", "ok", 4)],
    });
    expect(turns.map((t) => t.key)).toEqual(["u1", "u2"]);
  });

  it("renders known steering and results text inline", () => {
    const turns = buildTimeline({
      messages: [
        user("u1", "go", 1),
        assistant("a1", [toolUse("c1", "Bash")], 2),
        user("r1", [toolResult("c1", "ok"), text("also check tests")], 3),
        user("s1", "and docs", 4),
      ],
      turnStarts: { u1: "t1" },
      steeringIds: { s1: true },
    });
    expect(turns).toHaveLength(1);
    expect(kinds(turns[0].items)).toEqual(["tool", "steer", "steer"]);
  });

  it("treats a prompt sent before the turn finished as steering", () => {
    const turns = buildTimeline({
      messages: [user("u1", "go", 1), user("s1", "faster", 5), assistant("a1", "ok", 6), user("u2", "next", 20)],
      turns: [turnRecord({ messageId: "u1", finishedAt: 10 })],
    });
    expect(turns.map((t) => t.key)).toEqual(["u1", "u2"]);
    expect(kinds(turns[0].items)).toEqual(["steer", "text"]);
  });

  it("marks the running turn active and treats later prompts as steering while busy", () => {
    const turns = buildTimeline({
      messages: [user("u1", "old", 1), user("u2", "new", 20), user("s1", "btw", 21)],
      turns: [turnRecord({ messageId: "u1", finishedAt: 10 })],
      busy: true,
    });
    expect(turns.map((t) => t.key)).toEqual(["u1", "u2"]);
    expect(turns[1].active).toBe(true);
    expect(kinds(turns[1].items)).toEqual(["steer"]);
  });

  it("uses the active turn id when it is known", () => {
    const turns = buildTimeline({
      messages: [user("u1", "go", 1), user("s1", "more", 2)],
      turnStarts: { u1: "t1" },
      activeMessageId: "u1",
    });
    expect(turns[0].active).toBe(true);
    expect(kinds(turns[0].items)).toEqual(["steer"]);
  });

  it("hides reminder-only user messages", () => {
    const turns = buildTimeline({
      messages: [user("u1", "go", 1), user("x", "<system-reminder>todo nudge</system-reminder>", 2)],
      turnStarts: { u1: "t1" },
    });
    expect(turns).toHaveLength(1);
    expect(turns[0].items).toEqual([]);
  });

  it("collects leading content and local cards", () => {
    const turns = buildTimeline({
      messages: [assistant("a0", "summary so far", 1), user("u1", "hi", 2)],
      outputs: [{ id: 2, name: "help", markdown: "x", afterMessageId: null, at: 0 }],
      errors: [{ id: 3, message: "bad", afterMessageId: "u1", at: 0 }],
    });
    expect(turns.map((t) => t.key)).toEqual(["lead", "u1"]);
    expect(kinds(turns[0].items)).toEqual(["output", "text"]);
    expect(kinds(turns[1].items)).toEqual(["error"]);
  });

  it("drops local cards anchored to missing messages", () => {
    const turns = buildTimeline({
      messages: [user("u1", "hi", 1)],
      outputs: [{ id: 1, name: "x", markdown: "y", afterMessageId: "gone", at: 0 }],
    });
    expect(turns[0].items).toEqual([]);
  });

  it("places compaction markers before the first kept message", () => {
    const marker = { keepFrom: "u2", summary: "s", tokensBefore: 100, tokensAfter: 10, createdAt: 5 };
    const turns = buildTimeline({
      messages: [user("u1", "a", 1), assistant("a1", "b", 2), user("u2", "c", 6)],
      compactions: [marker],
    });
    expect(kinds(turns[0].items)).toEqual(["text", "compaction"]);
    const loose = { ...marker, keepFrom: null };
    const after = buildTimeline({ messages: [user("u1", "a", 1), user("u2", "c", 9)], compactions: [loose] });
    expect(kinds(after[0].items)).toEqual(["compaction"]);
  });

  it("marks a compaction restored when the task packet went out before the next one", () => {
    const marker = (createdAt: number) => ({ keepFrom: null, summary: "s", tokensBefore: 100, tokensAfter: 10, createdAt });
    const turns = buildTimeline({ messages: [user("u1", "a", 1)], compactions: [marker(5), marker(20)], restores: [7] });
    expect(turns[0].items).toMatchObject([
      { kind: "compaction", restored: true },
      { kind: "compaction", restored: false },
    ]);
  });

  it("places plan traces after the last message before them, in time order with dividers", () => {
    const trace = (key: string, at: number) => ({
      key,
      planId: "P1",
      kind: "approved" as const,
      at,
      text: "Plan approved",
      detail: null,
      tone: "quiet" as const,
      taskId: null,
      entryId: null,
    });
    const turns = buildTimeline({
      messages: [user("u1", "a", 1), assistant("a1", "b", 2), user("u2", "c", 10)],
      compactions: [{ keepFrom: null, summary: "s", tokensBefore: 100, tokensAfter: 10, createdAt: 4 }],
      traces: [trace("late", 12), trace("early", 3)],
    });
    expect(turns[0].items.map((i) => i.key)).toEqual(["a1:0", "early", "compact:0:4"]);
    expect(turns[1].items.map((i) => i.key)).toEqual(["late"]);
  });

  it("marks redacted thinking and skips empty text", () => {
    const turns = buildTimeline({
      messages: [
        user("u1", "q", 1),
        assistant("a1", [{ type: "redactedThinking", data: "x" }, text("  "), text("answer")], 2),
      ],
    });
    expect(turns[0].items).toMatchObject([{ kind: "thinking", redacted: true }, { kind: "text", text: "answer" }]);
  });
});
