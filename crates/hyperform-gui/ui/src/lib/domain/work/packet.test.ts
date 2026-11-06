import { describe, expect, it } from "vitest";
import type { TaskPacket } from "../../protocol/TaskPacket";
import { criterion, entry, task } from "./fixtures";
import { latestPackets, packetSections, packetSize, sentReason } from "./packet";

describe("task packet", () => {
  it("keeps the newest packet per agent, the main agent first", () => {
    const latest = latestPackets([
      { agentId: "agt_1", taskId: "a", tokens: 1, reason: "task", at: 30 },
      { agentId: "main", taskId: "b", tokens: 1, reason: "task", at: 10 },
      { agentId: "main", taskId: "c", tokens: 1, reason: "compaction", at: 20 },
    ]);
    expect(latest.map((p) => [p.agentId, p.taskId])).toEqual([
      ["main", "c"],
      ["agt_1", "a"],
    ]);
    expect(sentReason("compaction")).toBe("after compaction");
    expect(sentReason("later")).toBe("later");
  });

  it("lists what the model was given and why each entry made it in", () => {
    const packet: TaskPacket = {
      planId: "P1",
      revision: 4,
      goal: "Add offline sync",
      invariants: [{ id: "i1", kind: "invariant", text: "Never lose a write", provenance: "user", status: "active", revision: 1 }],
      requirements: [],
      task: task("t1", { title: "Replay on reconnect", acceptance: [criterion("c1", { text: "Sync tests pass", status: "met" })] }),
      parent: { id: "p", title: "Implementation", status: { type: "active" } },
      dependencies: [{ id: "d", title: "Queue writes", status: { type: "done" }, result: "Queue in IndexedDB" }],
      entries: [{ entry: entry("f1", { type: "discovery", text: "C is generated", source: null }), reason: "scope", tokens: 40 }],
      evidence: [{ criterionId: "c1", evidence: { type: "note", text: "ran" }, stale: true }],
      blockers: [{ type: "held" }],
      scope: ["src/sync/**"],
      resume: null,
      nextReady: null,
      tokens: 3_200,
      budget: 4_000,
      dropped: 2,
    };
    const sections = packetSections(packet, {});
    expect(sections.map((s) => s.title)).toEqual(["Goal", "Invariants", "Task", "Dependencies", "Notebook", "Evidence", "Blockers", "Scope"]);
    expect(sections[2].lines[0]).toMatchObject({ text: "Replay on reconnect", why: "under “Implementation”" });
    expect(sections[3].lines[0].why).toBe("Queue in IndexedDB");
    expect(sections[4].lines[0]).toMatchObject({ text: "C is generated", why: "shares its files", tokens: 40 });
    expect(sections[5].lines[0].why).toBe("stale");
    expect(sections[6].lines[0].text).toBe("on hold");
    expect(packetSize(packet)).toBe("3.2k of 4.0k tokens · 2 left out");
  });
});
