import { describe, expect, it } from "vitest";
import type { BlockReason } from "../../protocol/BlockReason";
import type { PlanItem } from "../../protocol/PlanItem";
import type { TaskRun } from "../../protocol/TaskRun";
import { criterion, finding, header, task, work } from "./fixtures";
import { needLine, needsStrip, needsYouCount, needsYouItems } from "./needsYou";

const run = (over: Partial<TaskRun>): TaskRun => ({
  dispatch: null,
  integration: null,
  report: null,
  forgivenAttempts: 0,
  serial: false,
  ...over,
});
const blocked = (reason: BlockReason) => ({ type: "blocked" as const, reason });

describe("needs you", () => {
  it("is empty while nothing waits on you", () => {
    expect(needsYouItems(work([task("a"), task("b", { status: { type: "active" } })]))).toEqual([]);
  });

  it("orders by urgency: conflicts, pending applies, stopped tasks, proposals, then high findings", () => {
    const plan = header({
      requirements: [{ id: "r1", kind: "requirement", text: "Works offline", provenance: "agent", status: "proposed", revision: 2 }],
    });
    const view = work(
      [
        task("p", { status: { type: "proposed" } }),
        task("u", { status: blocked({ type: "unreported" }) }),
        task("x", {
          status: { type: "active" },
          run: run({
            integration: { type: "pendingApply", worktree: "/repo/.z-engine/worktrees/a2" },
            dispatch: { sessionId: "S1", agentId: "a2", worktree: "/w", attempt: 1, at: 1 },
          }),
        }),
        task("c", {
          status: blocked({ type: "mergeConflict", detail: "src/a.rs" }),
          run: run({ integration: { type: "conflict", paths: ["src/a.rs"], worktree: "/repo/.z-engine/worktrees/a1" } }),
        }),
        task("d", { status: blocked({ type: "dependency", taskId: "c" }) }),
      ],
      {
        plan,
        audit: [
          finding("low", { severity: "low" }),
          finding("high", { severity: "high" }),
          finding("gone", { severity: "high", status: "dismissed" }),
          finding("added", { severity: "high", status: "added" }),
        ],
      },
    );
    const items = needsYouItems(view);
    expect(items.map((i) => i.key)).toEqual(["conflict:c", "apply:x", "retry:u", "item:r1", "task:p", "finding:high"]);
    expect(items[0]).toMatchObject({ paths: ["src/a.rs"], worktree: "/repo/.z-engine/worktrees/a1" });
    expect(items[1]).toMatchObject({ agentId: "a2" });
  });

  it("leaves medium and low findings to the findings list, as the kernel's count does", () => {
    const view = work([], {
      audit: [finding("m", { severity: "medium" }), finding("l", { severity: "low" }), finding("h", { severity: "high" })],
    });
    const items = needsYouItems(view);
    expect(items.map((i) => i.key)).toEqual(["finding:h"]);
    expect(needsYouCount(items)).toBe(1);
    expect(needsYouItems(work([], { audit: [finding("m", { severity: "medium" })] }))).toEqual([]);
  });

  it("counts every item but the replan note, which waits on the change set", () => {
    const view = work([task("h1"), task("p", { status: { type: "proposed" } })], {
      plan: header({ status: "replanning" }),
      replanHeld: ["h1"],
      audit: [finding("h", { severity: "high" })],
    });
    const items = needsYouItems(view);
    expect(items.map((i) => i.kind)).toEqual(["replan", "task", "finding"]);
    expect(needsYouCount(items)).toBe(2);
  });

  it("notes the tasks a plan change holds, after stopped work and before proposals", () => {
    const view = work(
      [task("h1", { order: 1 }), task("h2", { order: 2 }), task("p", { status: { type: "proposed" } }), task("u", { status: blocked({ type: "unreported" }) })],
      { plan: header({ status: "replanning" }), replanHeld: ["h2", "h1"] },
    );
    const items = needsYouItems(view);
    expect(items.map((i) => i.kind)).toEqual(["replan", "retry", "task"]);
    const replan = items[0];
    expect(replan.kind === "replan" && replan.tasks.map((t) => t.id)).toEqual(["h1", "h2"]);
    expect(needLine(replan)).toBe("2 tasks wait for you to accept the plan change");
    expect(needsYouItems(work([task("h1")], { replanHeld: ["h1"] }))).toEqual([]);
  });

  it("offers to retry a task whose change was discarded at the merge card", () => {
    const discarded = task("z", { title: "Badge", status: blocked({ type: "changeDiscarded" }) });
    const [item] = needsYouItems(work([discarded]));
    expect(item).toMatchObject({ kind: "retry", reason: "its change was discarded" });
    expect(needLine(item)).toBe("“Badge” stopped: its change was discarded");
  });

  it("asks to run a verifying task's checks again when one did not run", () => {
    const method = { type: "check" as const, kind: "test" as const };
    const stuck = task("v", { title: "Retry", status: { type: "verifying" }, acceptance: [criterion("c1", { method })] });
    const [item] = needsYouItems(work([stuck]));
    expect(item).toMatchObject({ kind: "recheck", key: "recheck:v" });
    expect(needLine(item)).toBe("“Retry” waits on checks that did not run or are stale");
  });

  it("leaves closed tasks and settled proposals out", () => {
    const plan = header({
      invariants: [{ id: "i1", kind: "invariant", text: "No data loss", provenance: "user", status: "active", revision: 1 }],
    });
    expect(needsYouItems(work([task("a", { status: { type: "cancelled" } })], { plan }))).toEqual([]);
  });

  it("never folds what the plan waits on, but turns findings past the first two into a count", () => {
    const view = work(
      [task("p", { status: { type: "proposed" } }), task("q", { status: { type: "proposed" } })],
      { audit: ["f1", "f2", "f3", "f4"].map((id) => finding(id, { severity: "high" })) },
    );
    const strip = needsStrip(needsYouItems(view));
    expect(strip.shown.map((i) => i.key)).toEqual(["task:p", "task:q", "finding:f1", "finding:f2"]);
    expect(strip.moreFindings).toBe(2);
    expect(needsStrip([]).moreFindings).toBe(0);
  });

  it("words each item in one line", () => {
    const item: PlanItem = { id: "r1", kind: "invariant", text: "No data loss", provenance: "agent", status: "proposed", revision: 1 };
    expect(needLine({ kind: "item", key: "k", item })).toBe("Proposed invariant: No data loss");
    expect(needLine({ kind: "conflict", key: "k", task: task("a", { title: "Sync" }), paths: [], worktree: null })).toBe(
      "Merge conflict in “Sync”",
    );
  });
});
