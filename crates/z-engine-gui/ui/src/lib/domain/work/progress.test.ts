import { describe, expect, it } from "vitest";
import { criterion, task, work } from "./fixtures";
import { criteriaMet, currentTask, doneTimes, planProgress, planRow, stepLabel, stepProgress, tasksDoneIn } from "./progress";

describe("plan progress", () => {
  it("counts done of total, leaving out what is not needed, replaced or only proposed", () => {
    const tasks = [
      task("a", { status: { type: "done" } }),
      task("b"),
      task("c", { status: { type: "cancelled" } }),
      task("d", { status: { type: "superseded", by: "b" } }),
      task("e", { status: { type: "proposed" } }),
    ];
    expect(planProgress(tasks)).toEqual({ done: 1, total: 2 });
    expect(planProgress(Object.fromEntries(tasks.map((t) => [t.id, t])))).toEqual({ done: 1, total: 2 });
  });

  it("prefers the task bound to this chat's main agent as the current one", () => {
    const helper = task("h", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "agt_1" } });
    const mine = task("m", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "main" } });
    expect(currentTask(work([helper, mine]), "S1")?.id).toBe("m");
    expect(currentTask(work([helper]), "S1")?.id).toBe("h");
    expect(currentTask(work([task("v", { status: { type: "verifying" } })]), "S1")?.id).toBe("v");
    expect(currentTask(null, "S1")).toBeNull();
  });

  it("names the step in progress, else what the task is doing", () => {
    const steps = task("a", {
      activeForm: "Replaying on reconnect",
      steps: [
        { content: "Write the queue", activeForm: "Writing the queue", status: "completed" },
        { content: "Run the sync tests", activeForm: "Running the sync tests", status: "in_progress" },
      ],
    });
    expect(stepLabel(steps)).toBe("Running the sync tests");
    expect(stepProgress(steps)).toEqual({ done: 1, total: 2 });
    expect(stepLabel(task("b", { activeForm: "Replaying on reconnect" }))).toBe("Replaying on reconnect");
  });

  it("counts criteria met or waived", () => {
    const t = task("a", { acceptance: [criterion("c1", { status: "met" }), criterion("c2", { status: "waived" }), criterion("c3")] });
    expect(criteriaMet(t)).toEqual({ done: 2, total: 3 });
  });

  it("counts the tasks a turn finished", () => {
    const tasks = { a: task("a", { status: { type: "done" }, updatedAt: 150 }), b: task("b", { updatedAt: 160 }) };
    expect(tasksDoneIn(doneTimes(tasks), 100, 200)).toBe(1);
    expect(tasksDoneIn(doneTimes(tasks), 200, 300)).toBe(0);
  });

  it("writes the island card's Plan row", () => {
    const view = work([task("a", { status: { type: "done" } }), task("b", { title: "Replay on reconnect", status: { type: "active" } })]);
    expect(planRow(view, "S1")).toEqual({ progress: { done: 1, total: 2 }, label: "1 of 2 · Replay on reconnect" });
  });
});
