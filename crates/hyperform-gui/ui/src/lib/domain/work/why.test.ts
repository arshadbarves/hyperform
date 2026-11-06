import { describe, expect, it } from "vitest";
import type { WhyStep } from "../../protocol/WhyStep";
import { entry, task } from "./fixtures";
import { actorLabel, eventText, whyLines } from "./why";

const discovery = entry("f1", { type: "discovery", text: "C is generated", source: "EXP-31" });
const change = entry("c1", {
  type: "planChange",
  cause: "discovery",
  source: "f1",
  original: "Modify C",
  new: "Modify D",
  reason: "Modify D, not C: C is generated",
  evidence: [
    { type: "note", text: "generation header" },
    { type: "note", text: "build output" },
  ],
  affected: ["t1"],
});

describe("why", () => {
  it("names who acted", () => {
    expect(actorLabel({ type: "user" })).toBe("You");
    expect(actorLabel({ type: "agent", sessionId: "S1", agentId: "main" })).toBe("The agent");
    expect(actorLabel({ type: "agent", sessionId: "S1", agentId: "agt_1" })).toBe("A helper");
    expect(actorLabel({ type: "kernel" })).toBe("Automatic");
  });

  it("says what happened in words", () => {
    const tasks = { t2: task("t2", { title: "Research" }) };
    expect(eventText({ type: "taskTransitioned", taskId: "t1", from: { type: "pending" }, to: { type: "active" } }, tasks)).toBe(
      "Pending → In progress",
    );
    expect(eventText({ type: "dependencyAdded", taskId: "t1", dependsOn: "t2" }, tasks)).toBe("Now after “Research”");
    const patch = { title: "New", activeForm: null, description: null, kind: null, priority: null, scope: { files: ["a"], symbols: [] }, notes: null, acceptance: null, requirements: null, runAlone: null, result: null };
    expect(eventText({ type: "taskEdited", taskId: "t1", patch }, tasks)).toBe("Changed title, scope");
    expect(eventText({ type: "checkpointLinked", fingerprint: "f", checkpoint: null }, tasks)).toBe("Checkpoint linked");
  });

  it("carries the plan change and discovery behind a step, with its evidence", () => {
    const steps: WhyStep[] = [
      {
        revision: 8,
        at: 100,
        actor: { type: "agent", sessionId: "S1", agentId: "main" },
        cause: { type: "change", entryId: "c1" },
        event: { type: "taskCreated", task: task("t1") },
        entries: [change, discovery],
      },
    ];
    expect(whyLines(steps, {})).toEqual([
      {
        key: "8:0",
        revision: 8,
        at: 100,
        who: "The agent",
        what: "Created",
        cause: "a plan change",
        because: ["Discovery: Modify D, not C: C is generated", "Discovery: C is generated"],
        evidence: ["generation header", "build output"],
      },
    ]);
  });

  it("says a resume once, with what resuming found", () => {
    const resumed = entry("r1", {
      type: "planChange",
      cause: "resume",
      source: null,
      original: "revision 4",
      new: "",
      reason: "Resumed: 1 task was interrupted",
      evidence: [],
      affected: ["t1"],
    });
    const step: WhyStep = {
      revision: 5,
      at: 200,
      actor: { type: "kernel" },
      cause: { type: "resume" },
      event: { type: "taskTransitioned", taskId: "t1", from: { type: "active" }, to: { type: "pending" } },
      entries: [resumed],
    };
    expect(whyLines([step], {})[0]).toMatchObject({ who: "Automatic", cause: "resume", because: ["Resumed: 1 task was interrupted"] });
  });
});
