import { describe, expect, it } from "vitest";
import { finding, header, health, task } from "./fixtures";
import { coverageLine, findingGroups, findingKind, findingRefs, findingStatus } from "./health";

describe("plan health", () => {
  it("reads as counts and a level, never a percentage", () => {
    const audit = [finding("h", { severity: "high" }), finding("m")];
    expect(coverageLine(health({ requirements: 5, requirementsCovered: 4, tasksDone: 2, evidence: "fair", openFindings: 2 }), audit)).toEqual({
      text: "4 of 5 requirements covered · evidence Fair · 2 findings",
      tone: "attention",
    });
  });

  it("leaves evidence out until something is done, and is quiet without findings", () => {
    expect(coverageLine(health({ requirements: 1, requirementsCovered: 1, evidence: "fair" }), [])).toEqual({
      text: "1 of 1 requirement covered",
      tone: "quiet",
    });
    expect(coverageLine(health(), [])).toEqual({ text: "No open findings", tone: "quiet" });
    expect(coverageLine(health({ openFindings: 1 }), [finding("m")]).text).toBe("1 finding");
  });

  it("counts medium and low findings quietly, since only high ones wait on you", () => {
    const audit = [finding("m", { severity: "medium" }), finding("l", { severity: "low" }), finding("h", { severity: "high", status: "dismissed" })];
    expect(coverageLine(health({ openFindings: 2 }), audit)).toEqual({ text: "2 findings", tone: "quiet" });
  });

  it("groups open findings by severity and keeps the settled ones apart", () => {
    const audit = [
      finding("a", { severity: "low" }),
      finding("b", { severity: "high" }),
      finding("c", { severity: "high", status: "added" }),
      finding("d", { status: "dismissed" }),
      finding("e", { status: "resolved" }),
    ];
    const groups = findingGroups(audit);
    expect(groups.open.map((g) => [g.label, g.findings.map((f) => f.id)])).toEqual([
      ["High", ["b"]],
      ["Low", ["a"]],
    ]);
    expect(groups.added.map((f) => f.id)).toEqual(["c"]);
    expect(groups.settled.map((f) => f.id)).toEqual(["d", "e"]);
    expect(audit.map((f) => findingStatus(f.status))).toEqual([null, null, "added to the plan", "dismissed", "resolved"]);
    expect(findingKind("uncoveredRequirement")).toBe("Requirement not covered");
  });

  it("links a finding to its tasks, once each, and names requirements and files", () => {
    const plan = header({
      requirements: [{ id: "r1", kind: "requirement", text: "Works offline", provenance: "user", status: "active", revision: 1 }],
    });
    const refs = findingRefs(
      finding("f", {
        refs: [
          { type: "item", itemId: "r1" },
          { type: "task", taskId: "t1" },
          { type: "criterion", taskId: "t1", criterionId: "c1" },
          { type: "file", path: "src/store/db.rs" },
        ],
      }),
      { t1: task("t1", { title: "Queue writes" }) },
      plan,
    );
    expect(refs).toEqual([
      { key: "item:r1", label: "Works offline", taskId: null },
      { key: "task:t1", label: "Queue writes", taskId: "t1" },
      { key: "file:src/store/db.rs", label: "src/store/db.rs", taskId: null },
    ]);
  });
});
