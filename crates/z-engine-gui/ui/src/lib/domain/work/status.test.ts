import { describe, expect, it } from "vitest";
import type { BlockReason } from "../../protocol/BlockReason";
import { criterion, task } from "./fixtures";
import {
  blockKind,
  blockedText,
  dependencyBadge,
  fromWeb,
  isPinned,
  needsYou,
  taskActions,
  taskLook,
  taskState,
} from "./status";

const done = task("a", { title: "Research", status: { type: "done" } });
const open = task("b", { title: "Queue writes" });

describe("task status", () => {
  it("derives Ready only for a pending task with every dependency done in an active plan", () => {
    const tasks = { a: done, c: task("c", { dependsOn: ["a"] }) };
    expect(taskState(tasks.c, tasks, "active")).toBe("ready");
    expect(taskState(tasks.c, tasks, "draft")).toBe("pending");
    expect(taskState({ ...tasks.c, held: true }, tasks, "active")).toBe("held");
  });

  it("derives Waiting while a dependency is open", () => {
    const tasks = { b: open, c: task("c", { dependsOn: ["b"] }) };
    expect(taskLook(tasks.c, tasks, "active")).toMatchObject({ state: "waiting", tone: "quiet", detail: "after Queue writes" });
  });

  it("colors by tone: working, done, needs you, failed", () => {
    expect(taskLook(task("x", { status: { type: "active" } }), {}, "active")).toMatchObject({ tone: "working", label: "In progress" });
    expect(taskLook(done, {}, "active")).toMatchObject({ tone: "ok", label: "Done" });
    expect(taskLook(task("x", { status: { type: "failed", reason: "tests" } }), {}, "active")).toMatchObject({
      tone: "danger",
      detail: "tests",
    });
    const blocked = task("x", { status: { type: "blocked", reason: { type: "dependency", taskId: "a" } } });
    expect(taskLook(blocked, { a: done }, "active")).toMatchObject({ tone: "attention", detail: "waits on “Research”" });
  });

  it("names a replacement and a cancelled task plainly", () => {
    const replaced = task("x", { status: { type: "superseded", by: "b" } });
    expect(taskLook(replaced, { b: open }, "active")).toMatchObject({ label: "Replaced", detail: "replaced by “Queue writes”" });
    expect(taskLook(task("x", { status: { type: "cancelled" } }), {}, "active").label).toBe("Not needed");
  });

  it("badges dependencies: after one, waits on more, nothing once done", () => {
    const tasks = { a: done, b: open, d: task("d", { title: "Other" }) };
    expect(dependencyBadge(task("c", { dependsOn: ["b"] }), tasks)).toBe("after Queue writes");
    expect(dependencyBadge(task("c", { dependsOn: ["b", "d"] }), tasks)).toBe("waits on 2");
    expect(dependencyBadge(task("c", { dependsOn: ["a", "missing"] }), tasks)).toBeNull();
  });

  it("pins failed, blocked and waiting-on-you tasks", () => {
    expect(isPinned(task("x", { status: { type: "failed", reason: "" } }))).toBe(true);
    expect(isPinned(task("x", { status: { type: "proposed" } }))).toBe(true);
    const userCheck = task("x", {
      status: { type: "verifying" },
      acceptance: [criterion("c1", { method: { type: "user" } })],
    });
    expect(needsYou(userCheck)).toBe(true);
    expect(taskLook(userCheck, {}, "active")).toMatchObject({ tone: "attention", detail: "waits on your check" });
    expect(isPinned(open)).toBe(false);
  });

  it("offers hold, retry and not needed only where they apply", () => {
    expect(taskActions(open, "ready", false)).toEqual({
      rename: true,
      hold: true,
      release: false,
      retry: false,
      cancel: true,
      runAlone: true,
      accept: false,
    });
    expect(taskActions({ ...open, held: true }, "held", false)).toMatchObject({ hold: false, release: true });
    const running = task("r", { status: { type: "active" } });
    expect(taskActions(running, "active", false)).toMatchObject({ hold: true, release: false });
    expect(taskActions({ ...running, held: true }, "active", false)).toMatchObject({ hold: false, release: true });
    const failed = task("x", { status: { type: "failed", reason: "" } });
    expect(taskActions(failed, "failed", false)).toMatchObject({ retry: true, hold: false });
    expect(taskActions(done, "done", false)).toMatchObject({ rename: false, cancel: false });
    expect(Object.values(taskActions(open, "ready", true)).some(Boolean)).toBe(false);
  });

  it("offers Accept and Dismiss instead of Not needed for a proposed task", () => {
    const proposed = task("p", { status: { type: "proposed" } });
    expect(taskActions(proposed, "proposed", false)).toMatchObject({ accept: true, cancel: false });
    expect(taskActions(proposed, "proposed", true).accept).toBe(false);
  });

  it("marks what a turn that read web content added", () => {
    expect(fromWeb(task("w", { provenance: "research" }))).toBe(true);
    expect(fromWeb(open)).toBe(false);
  });

  it("says which dependency was replaced, and by what", () => {
    const tasks = { a: done, b: open };
    const reason: BlockReason = { type: "dependencySuperseded", taskId: "a", by: "b" };
    expect(blockedText(reason, tasks)).toBe("depends on “Research”, which was replaced by “Queue writes”");
  });

  it("words a change discarded at the merge card", () => {
    expect(blockedText({ type: "changeDiscarded" }, {})).toBe("its change was discarded");
  });

  it("says a missing report the way the engine tells the model", () => {
    expect(blockedText({ type: "unreported" }, {})).toBe("ended without a report");
  });

  it("words a block reason from a newer engine from its own fields, else its name", () => {
    const unknown = (reason: Record<string, unknown>) => reason as unknown as BlockReason;
    expect(blockedText(unknown({ type: "patchDiscarded" }), {})).toBe("its change was discarded");
    expect(blockedText(unknown({ type: "waitingOnReview" }), {})).toBe("waiting on review");
    expect(blockedText(unknown({ type: "somethingNew", detail: "the user said no" }), {})).toBe("the user said no");
  });

  it("sorts blocks by who clears them", () => {
    expect(blockKind({ type: "dependency", taskId: "a" })).toBe("dependency");
    expect(blockKind({ type: "dependencySuperseded", taskId: "a", by: "b" })).toBe("dependency");
    expect(blockKind({ type: "mergeConflict", detail: "" })).toBe("conflict");
    expect(blockKind({ type: "unreported" })).toBe("retry");
    expect(blockKind({ type: "changeDiscarded" })).toBe("retry");
    expect(blockKind({ type: "somethingNew" } as unknown as BlockReason)).toBe("retry");
    expect(blockKind({ type: "other", text: "x" })).toBe("other");
  });
});
