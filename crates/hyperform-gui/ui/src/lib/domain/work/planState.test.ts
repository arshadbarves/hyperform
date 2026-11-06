import { describe, expect, it } from "vitest";
import type { EvidenceRef } from "../../protocol/EvidenceRef";
import { criterion, task, work } from "./fixtures";
import { doneLines, stateBanner } from "./planState";

const passed = (label: string): EvidenceRef => ({
  type: "check",
  sessionId: "S1",
  recordId: `rec_${label}`,
  passed: true,
  fingerprint: "f",
  kind: "test",
  label,
  at: 1,
});

describe("plan state banners", () => {
  it("pauses calmly, offering Stop only while helpers still wind down", () => {
    const owner = { sessionId: "S1", agentId: "agt_1" };
    const running = stateBanner("paused", work([task("a", { status: { type: "active" }, owner })]));
    expect(running).toMatchObject({ title: "Paused", resume: true, stop: true, reopen: false });
    expect(running?.text).toContain("stop at their next step");
    const idle = stateBanner("paused", work([task("a")]));
    expect(idle).toMatchObject({ stop: false, text: "Nothing runs until you resume. The work so far is kept." });
    const pointer = { sessionId: "S1", agentId: "agt_1", worktree: null, note: "" };
    expect(stateBanner("paused", work([task("a", { resume: pointer }), task("b", { resume: pointer })]))?.text).toBe(
      "Nothing runs until you resume; 2 interrupted tasks pick up where they stopped.",
    );
  });

  it("sums up a done plan: tasks, how criteria were shown to hold, the checks and what you waived", () => {
    const view = work([
      task("a", {
        status: { type: "done" },
        acceptance: [
          criterion("c1", { method: { type: "check", kind: "test" }, status: "met", evidence: [passed("cargo test")] }),
          criterion("c2", { method: { type: "check", kind: "lint" }, status: "met", evidence: [passed("cargo clippy")] }),
          criterion("c3", { status: "met" }),
        ],
      }),
      task("b", {
        status: { type: "done" },
        acceptance: [criterion("c1", { method: { type: "user" }, status: "met" }), criterion("c2", { status: "waived" })],
      }),
      task("c", { status: { type: "cancelled" } }),
    ]);
    expect(doneLines(view)).toEqual([
      "2 tasks done · 1 not needed",
      "4 criteria met: 2 by checks, 1 by inspection, 1 by you",
      "Checks passed: cargo test, cargo clippy",
      "1 criterion waived by you",
    ]);
    expect(stateBanner("done", view)).toMatchObject({ title: "Done", reopen: true, resume: false });
  });

  it("keeps an abandoned plan read-only, with Reopen", () => {
    expect(stateBanner("abandoned", work())).toMatchObject({ title: "Abandoned", reopen: true, stop: false, lines: [] });
    expect(stateBanner("building", work())).toBeNull();
  });
});
