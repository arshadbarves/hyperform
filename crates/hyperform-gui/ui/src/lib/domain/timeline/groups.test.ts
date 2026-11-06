import { describe, expect, it } from "vitest";
import { user } from "../testFixtures";
import {
  answerText,
  groupTurnItems,
  runLine,
  splitTrailingCompactions,
  summarizeRun,
  workLine,
  workSection,
  type RunCall,
} from "./groups";
import type { TimelineItem } from "./turns";

const tool = (id: string, name: string, input: Record<string, unknown> = {}): TimelineItem => ({
  kind: "tool",
  key: `tool:${id}`,
  use: { callId: id, name, input: input as never },
});
const text = (id: string, value = "hi"): TimelineItem => ({ kind: "text", key: id, messageId: id, text: value });
const thought = (id: string): TimelineItem => ({ kind: "thinking", key: id, text: "hmm", redacted: false });
const steer = (id: string): TimelineItem => ({ kind: "steer", key: id, message: user(id, "also this") });
const keys = (items: TimelineItem[]) => items.map((item) => item.key);

function call(name: string, input: Record<string, unknown> = {}, status: RunCall["status"] = "ok"): RunCall {
  return { name, input: input as never, status };
}

describe("groupTurnItems", () => {
  it("folds consecutive tool calls into one run and keeps text between runs", () => {
    const blocks = groupTurnItems([tool("a", "Read"), tool("b", "Grep"), text("t"), tool("c", "Bash"), tool("d", "Edit")]);
    expect(blocks.map((b) => (b.kind === "run" ? `run:${b.uses.map((u) => u.callId).join("")}` : b.item.kind))).toEqual([
      "run:ab",
      "text",
      "run:cd",
    ]);
  });

  it("leaves a lone call as its own card", () => {
    expect(groupTurnItems([tool("a", "Read"), text("t")]).map((b) => b.kind)).toEqual(["item", "item"]);
  });

  it("never folds agents, questions or plans, which stand on their own", () => {
    const blocks = groupTurnItems([tool("a", "Read"), tool("b", "Agent"), tool("c", "Read"), tool("d", "Grep")]);
    expect(blocks.map((b) => (b.kind === "run" ? "run" : b.item.kind))).toEqual(["tool", "tool", "run"]);
  });

  it("keys a run by its first call so it stays put while it grows", () => {
    const first = groupTurnItems([tool("a", "Read"), tool("b", "Read")]);
    const later = groupTurnItems([tool("a", "Read"), tool("b", "Read"), tool("c", "Bash")]);
    expect(first[0].key).toBe(later[0].key);
  });
});

describe("summarizeRun", () => {
  it("describes the work in plain words, counting distinct files", () => {
    const summary = summarizeRun([
      call("Read", { file_path: "a.rs" }),
      call("Read", { file_path: "b.rs" }),
      call("Read", { file_path: "a.rs" }),
      call("Grep", { pattern: "x" }),
      call("Grep", { pattern: "y" }),
      call("Edit", { file_path: "a.rs" }),
      call("Bash", { command: "npm test" }),
    ]);
    expect(summary.parts).toEqual(["read 2 files", "searched 2×", "edited 1 file", "ran 1 command"]);
    expect(runLine(summary)).toBe("Read 2 files · searched 2× · edited 1 file · ran 1 command");
    expect(summary.state).toBe("ok");
  });

  it("names the step in progress while the run is live", () => {
    const summary = summarizeRun([call("Read", { file_path: "a.rs" }), call("Bash", { command: "cargo test" }, "running")]);
    expect(summary).toMatchObject({ state: "running", current: "Running cargo test" });
  });

  it("never hides a failure or a denial behind the fold", () => {
    const summary = summarizeRun([
      call("Bash", { command: "a" }, "error"),
      call("Edit", { file_path: "x" }, "denied"),
      call("Read", { file_path: "y" }),
    ]);
    expect(summary).toMatchObject({ state: "failed", failed: 1, denied: 1 });
  });

  it("groups MCP calls by server and keeps other tools generic", () => {
    const summary = summarizeRun([call("mcp__github__list_prs"), call("mcp__github__get_pr"), call("Verify"), call("TodoWrite")]);
    expect(summary.parts).toEqual(["used github 2×", "used 1 other tool", "updated the plan"]);
  });
});

describe("workSection", () => {
  const turn = [thought("k"), text("n1", "Looking."), tool("a", "Read"), tool("b", "Bash"), text("n2", "All set."), text("n3", "Done.")];

  it("puts thinking, calls and narration in the work, and what follows the last call in the answer", () => {
    const section = workSection(turn, true);
    expect(keys(section.work)).toEqual(["k", "n1", "tool:a", "tool:b"]);
    expect(keys(section.answer)).toEqual(["n2", "n3"]);
  });

  it("folds only a finished turn that made two or more calls", () => {
    expect(workSection(turn, true).fold).toBe(true);
    expect(workSection(turn, false).fold).toBe(false);
    expect(workSection([thought("k"), tool("a", "Read"), text("t")], true).fold).toBe(false);
    expect(workSection([text("t")], true)).toMatchObject({ work: [], fold: false });
  });

  it("keeps agents, questions, plans, failures and steering in view under the fold", () => {
    const items = [tool("a", "Read"), tool("b", "Agent"), steer("s"), tool("c", "Bash"), tool("d", "ExitPlanMode"), text("t")];
    const section = workSection(items, true, (id) => id === "c");
    expect(keys(section.pinned)).toEqual(["tool:b", "s", "tool:c", "tool:d"]);
  });

  it("counts only the calls that fold into the line", () => {
    const section = workSection([tool("a", "Read"), tool("b", "AskUserQuestion"), tool("c", "Edit")], true);
    expect(section.uses.map((use) => use.callId)).toEqual(["a", "c"]);
  });
});

describe("workLine", () => {
  const summary = summarizeRun([call("Read", { file_path: "a" }), call("Bash", { command: "x" })]);

  it("leads with how long the turn worked", () => {
    expect(workLine(summary, 72_000)).toBe("Worked for 1m 12s · read 1 file · ran 1 command");
  });

  it("drops the time when the turn has no record", () => {
    expect(workLine(summary, null)).toBe("Worked · read 1 file · ran 1 command");
  });

  it("ends with the plan's tasks the turn finished", () => {
    expect(workLine(summary, null, 2)).toBe("Worked · read 1 file · ran 1 command · 2 tasks done");
    expect(workLine(summary, null, 1)).toBe("Worked · read 1 file · ran 1 command · 1 task done");
  });
});

describe("answerText", () => {
  it("copies the answer, joined as paragraphs", () => {
    const section = workSection([text("n", "Looking."), tool("a", "Read"), text("x", "One."), text("y", "Two.")], true);
    expect(answerText(section)).toBe("One.\n\nTwo.");
  });

  it("falls back to every reply when the turn ended on work", () => {
    expect(answerText(workSection([text("n", "Looking."), tool("a", "Read")], true))).toBe("Looking.");
  });
});

describe("splitTrailingCompactions", () => {
  const compaction = (id: string): TimelineItem => ({
    kind: "compaction",
    key: id,
    marker: { keepFrom: null, summary: "s", tokensBefore: 10, tokensAfter: 2, createdAt: 1 },
    restored: false,
  });
  const trace = (id: string): TimelineItem => ({
    kind: "trace",
    key: id,
    trace: { key: id, planId: "P1", kind: "revised", at: 1, text: "Plan revised", detail: null, tone: "quiet", taskId: null, entryId: null },
  });

  it("moves only the dividers after the last item out of the turn body", () => {
    const { body, after } = splitTrailingCompactions([text("t1"), compaction("c1"), tool("a", "Read"), text("t2"), compaction("c2")]);
    expect(keys(body)).toEqual(["t1", "c1", "tool:a", "t2"]);
    expect(keys(after)).toEqual(["c2"]);
  });

  it("moves trailing plan traces with them", () => {
    const { after } = splitTrailingCompactions([text("t1"), trace("r1"), compaction("c1")]);
    expect(keys(after)).toEqual(["r1", "c1"]);
  });
});
