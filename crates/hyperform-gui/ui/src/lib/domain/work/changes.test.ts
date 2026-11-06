import { describe, expect, it } from "vitest";
import type { EvidenceRef } from "../../protocol/EvidenceRef";
import { criterion, entry, header, task, work } from "./fixtures";
import { changeEffect, changeKind, changeLine, changeSet, replanBaseline } from "./changes";

const note: EvidenceRef = { type: "note", text: "passed" };
const planChange = (id: string, createdRevision: number, affected: string[]) =>
  entry(id, { type: "planChange", cause: "reconciliation", source: null, original: "", new: "", reason: "Scope moved", evidence: [], affected }, {
    createdRevision,
    revision: createdRevision,
  });

describe("change set", () => {
  it("counts from the approval seen live, else the move into replanning", () => {
    const view = work([], { plan: header({ revision: 9 }) });
    expect(replanBaseline({ ...view, moves: [{ revision: 4, from: "draft", to: "active", at: 1 }] })).toBe(4);
    expect(replanBaseline({ ...view, moves: [{ revision: 7, from: "active", to: "replanning", at: 1 }] })).toBe(6);
  });

  it("falls back to the revision before the newest plan change, else now", () => {
    const view = work([], { plan: header({ revision: 9 }), entries: { c: planChange("c", 8, []) } });
    expect(replanBaseline(view)).toBe(7);
    expect(replanBaseline(work([], { plan: header({ revision: 9 }) }))).toBe(9);
  });

  it("classifies tasks changed since: added, reopened, replaced, removed, changed", () => {
    expect(changeKind(task("a", { createdRevision: 6, revision: 6 }), 5)).toBe("added");
    expect(changeKind(task("b", { revision: 6, acceptance: [criterion("c1", { evidence: [note] })] }), 5)).toBe("reopened");
    expect(changeKind(task("c", { revision: 6, status: { type: "superseded", by: "a" } }), 5)).toBe("replaced");
    expect(changeKind(task("d", { revision: 6, status: { type: "cancelled" } }), 5)).toBe("removed");
    expect(changeKind(task("e", { revision: 6 }), 5)).toBe("changed");
    expect(changeKind(task("f", { revision: 5 }), 5)).toBeNull();
  });

  it("summarizes since you approved", () => {
    const view = work(
      [
        task("a", { createdRevision: 6, revision: 6 }),
        task("b", { createdRevision: 7, revision: 7 }),
        task("c", { revision: 6, result: "done before" }),
        task("d", { revision: 6, status: { type: "superseded", by: "a" } }),
        task("old"),
      ],
      { moves: [{ revision: 5, from: "draft", to: "active", at: 1 }], entries: { c1: planChange("c1", 6, ["a"]) } },
    );
    const set = changeSet(view);
    expect(set.line).toBe("2 tasks added, 1 reopened, 1 replaced");
    expect(set.tasks.map((t) => t.task.id)).toEqual(["a", "b", "c", "d"]);
    expect(set.changes.map((e) => e.id)).toEqual(["c1"]);
    expect(changeLine([])).toBe("");
  });

  it("says what one plan change did to its tasks", () => {
    const tasks = { a: task("a", { createdRevision: 6, revision: 6 }), d: task("d", { revision: 6, status: { type: "superseded", by: "a" } }) };
    expect(changeEffect(planChange("c1", 6, ["a", "d"]), tasks)).toBe("1 task added · 1 replaced");
  });
});
