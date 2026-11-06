import { describe, expect, it } from "vitest";
import type { TaskReport } from "../../protocol/TaskReport";
import type { TaskRun } from "../../protocol/TaskRun";
import { task } from "./fixtures";
import { attemptLine, integrationLine, resultView } from "./report";

function report(over: Partial<TaskReport> = {}): TaskReport {
  return {
    source: "worker",
    attempt: 1,
    summary: "Queued writes in IndexedDB",
    filesChanged: ["src/queue.ts"],
    diffKnown: true,
    symbolsChanged: [],
    discoveries: [],
    decisions: [],
    experiments: [],
    testsRun: [
      { type: "check", sessionId: "S1", recordId: "r1", passed: true, fingerprint: "f", kind: "test", label: "npm test", at: 1 },
    ],
    blockers: [],
    followUps: [],
    risks: [],
    confidence: "medium",
    claimed: { filesChanged: ["src/queue.ts", "src/extra.ts"], testsRun: ["npm test"] },
    provenance: "agent",
    at: 2,
    ...over,
  };
}

const run = (over: Partial<TaskRun>): TaskRun => ({
  dispatch: null,
  integration: null,
  report: null,
  forgivenAttempts: 0,
  serial: false,
  ...over,
});

describe("task result", () => {
  it("is absent with neither a report nor a result, and plain with only a result", () => {
    expect(resultView(task("a"))).toBeNull();
    expect(resultView(task("a", { result: "Done by hand" }))).toMatchObject({ summary: "Done by hand", claims: [] });
  });

  it("puts what the kernel observed first and the helper's own lists as claims", () => {
    const view = resultView(task("a", { run: run({ report: report() }) }));
    expect(view).toMatchObject({
      summary: "Queued writes in IndexedDB",
      files: ["src/queue.ts"],
      tests: [{ label: "npm test", passed: true }],
      claims: [
        { label: "Files", items: ["src/queue.ts", "src/extra.ts"] },
        { label: "Tests", items: ["npm test"] },
      ],
      confidence: "medium",
      fromWeb: false,
      unreported: false,
    });
  });

  it("says when the diff could not be read and when nothing was reported", () => {
    const unreported = report({ diffKnown: false, filesChanged: [], source: "unreported", summary: "" });
    expect(resultView(task("a", { run: run({ report: unreported }) }))).toMatchObject({ files: null, unreported: true, summary: null });
  });

  it("marks what a run that read web content learned as from the web", () => {
    const web = report({ provenance: "research", discoveries: ["The API caps at 5 MB"] });
    const view = resultView(task("a", { run: run({ report: web }) }));
    expect(view?.learned).toEqual([{ label: "Discoveries", items: ["The API caps at 5 MB"] }]);
    expect(view?.fromWeb).toBe(true);
  });

  it("words how the change came back", () => {
    expect(integrationLine(null)).toBeNull();
    expect(integrationLine({ type: "applied", files: ["a", "b"] })).toEqual({ text: "Applied to the project · 2 files", tone: "ok" });
    expect(integrationLine({ type: "conflict", paths: ["src/a.rs"], worktree: "/w" })?.tone).toBe("attention");
    expect(integrationLine({ type: "pendingApply", worktree: "/w" })?.text).toBe("The helper's change waits for you to apply it");
  });

  it("counts attempts against the budget, forgiving the ones before your retry", () => {
    const dispatch = { sessionId: "S1", agentId: "a1", worktree: null, attempt: 3, at: 1 };
    expect(attemptLine(task("a", { attempts: 2 }), 3)).toBeNull();
    expect(attemptLine(task("a", { attempts: 2, run: run({ dispatch }) }), 3)).toBe("Attempt 2 of 3");
    expect(attemptLine(task("a", { attempts: 3, run: run({ dispatch, forgivenAttempts: 2 }) }), 3)).toBe("Attempt 1 of 3");
    expect(attemptLine(task("a", { attempts: 1, run: run({ dispatch }) }), null)).toBe("Attempt 1");
  });
});
