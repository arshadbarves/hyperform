import { describe, expect, it } from "vitest";
import type { EvidenceRef } from "../../protocol/EvidenceRef";
import { criterion, task } from "./fixtures";
import { criterionLines, methodLabel, needsRecheck, staleRecords } from "./criteria";

const check = (recordId: string, passed: boolean): EvidenceRef => ({
  type: "check",
  sessionId: "S1",
  recordId,
  passed,
  fingerprint: "f",
  kind: "test",
  label: "cargo test",
  at: 500,
});

describe("criteria", () => {
  it("names how a criterion is shown to hold", () => {
    expect(methodLabel({ type: "check", kind: "test" })).toBe("Check · test");
    expect(methodLabel({ type: "user" })).toBe("Your check");
  });

  it("marks evidence stale from the latest reconciliation", () => {
    const findings = [
      { type: "staleEvidence" as const, taskId: "t1", files: ["a.rs"], records: ["r1"] },
      { type: "staleEvidence" as const, taskId: "t2", files: [], records: ["r2"] },
    ];
    expect([...staleRecords(findings, "t1")]).toEqual(["r1"]);
    const t = task("t1", {
      acceptance: [
        criterion("c1", { status: "met", evidence: [check("r1", true), { type: "diff", files: ["a.rs"], checkpoint: null, summary: "" }] }),
      ],
    });
    const [line] = criterionLines(t, findings);
    expect(line).toMatchObject({ label: "Met", tone: "ok", canWaive: false, canConfirm: false });
    expect(line.evidence).toMatchObject([
      { text: "cargo test passed", passed: true, at: 500, stale: true },
      { text: "a.rs", files: ["a.rs"], stale: false },
    ]);
  });

  it("lets you waive what does not hold, and confirm your own check", () => {
    const t = task("t1", {
      acceptance: [criterion("c1", { status: "failed" }), criterion("c2", { method: { type: "user" } }), criterion("c3", { status: "waived" })],
    });
    expect(criterionLines(t, []).map((l) => [l.canWaive, l.canConfirm])).toEqual([
      [true, false],
      [true, true],
      [false, false],
    ]);
  });

  it("offers to run a verifying task's checks again when one did not run or is stale", () => {
    const verifying = { type: "verifying" as const };
    const method = { type: "check" as const, kind: "test" as const };
    const ran = criterion("c1", { method, status: "met", evidence: [check("r1", true)] });
    const notRun = task("t1", { status: verifying, acceptance: [ran, criterion("c2", { method })] });
    expect(needsRecheck(notRun, [])).toBe(true);
    const settled = task("t1", { status: verifying, acceptance: [ran, criterion("c2")] });
    expect(needsRecheck(settled, [])).toBe(false);
    const stale = [{ type: "staleEvidence" as const, taskId: "t1", files: [], records: ["r1"] }];
    expect(needsRecheck(settled, stale)).toBe(true);
    expect(needsRecheck({ ...notRun, status: { type: "active" } }, [])).toBe(false);
  });
});
