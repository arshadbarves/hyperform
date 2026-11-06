import { describe, expect, it } from "vitest";
import type { Task } from "../../protocol/Task";
import { task } from "./fixtures";
import { ancestors, treeKeyAction, visibleRows, type TreeInput } from "./tree";

function map(...tasks: Task[]): Record<string, Task> {
  return Object.fromEntries(tasks.map((t) => [t.id, t]));
}

const research = task("r", { title: "Research", order: 0 });
const impl = task("i", { title: "Implementation", order: 1 });
const verify = task("v", { title: "Verification", order: 2 });

function rows(tasks: Record<string, Task>, over: Partial<TreeInput> = {}) {
  return visibleRows({ tasks, plan: "active", expanded: {}, showReplaced: false, review: false, ...over });
}

describe("task tree", () => {
  it("orders siblings and starts every group open under review", () => {
    const tasks = map(verify, impl, research, task("r1", { parent: "r", order: 0 }), task("i1", { parent: "i" }));
    const view = rows(tasks, { review: true, plan: "draft" });
    expect(view.rows.map((r) => r.task.id)).toEqual(["r", "r1", "i", "i1", "v"]);
    expect(view.rows[0]).toMatchObject({ depth: 0, pos: 1, size: 3, hasChildren: true, expanded: true, progress: { done: 0, total: 1 } });
    expect(view.rows[1]).toMatchObject({ depth: 1, parentId: "r", pos: 1, size: 1 });
  });

  it("while building, folds everything except the path to the active task", () => {
    const tasks = map(
      research,
      impl,
      task("r1", { parent: "r", status: { type: "done" } }),
      task("i1", { parent: "i", status: { type: "active" } }),
      task("i1a", { parent: "i1", status: { type: "active" } }),
    );
    const view = rows(tasks);
    expect(view.rows.map((r) => r.task.id)).toEqual(["r", "i", "i1", "i1a"]);
    expect(view.rows[0]).toMatchObject({ expanded: false, progress: { done: 1, total: 1 } });
  });

  it("never folds a failed, blocked or waiting-on-you task, whatever was toggled", () => {
    const tasks = map(impl, task("i1", { parent: "i", status: { type: "failed", reason: "tests" } }));
    const view = rows(tasks, { expanded: { i: false } });
    expect(view.rows.map((r) => r.task.id)).toEqual(["i", "i1"]);
    expect(view.rows[0]).toMatchObject({ expanded: true, canCollapse: false });
  });

  it("follows rows opened or closed by hand", () => {
    const tasks = map(research, task("r1", { parent: "r" }));
    expect(rows(tasks, { expanded: { r: true } }).rows).toHaveLength(2);
    expect(rows(tasks, { review: true, expanded: { r: false } }).rows).toHaveLength(1);
  });

  it("hides replaced tasks behind a count", () => {
    const tasks = map(research, task("old", { status: { type: "superseded", by: "r" } }));
    expect(rows(tasks)).toMatchObject({ replaced: 1 });
    expect(rows(tasks).rows.map((r) => r.task.id)).toEqual(["r"]);
    expect(rows(tasks, { showReplaced: true }).rows).toHaveLength(2);
  });

  it("treats a task whose parent is unknown as a root and walks the breadcrumb", () => {
    const tasks = map(research, task("r1", { parent: "r" }), task("r1a", { parent: "r1" }), task("orphan", { parent: "gone" }));
    expect(rows(tasks, { review: true }).rows.map((r) => r.task.id)).toContain("orphan");
    expect(ancestors("r1a", tasks).map((t) => t.id)).toEqual(["r", "r1"]);
  });
});

describe("tree keys", () => {
  const tasks = map(research, task("r1", { parent: "r" }), impl);
  const open = rows(tasks, { review: true }).rows;

  it("moves with the arrows, Home and End", () => {
    expect(treeKeyAction(open, 0, "ArrowDown")).toEqual({ type: "focus", index: 1 });
    expect(treeKeyAction(open, 0, "ArrowUp")).toBeNull();
    expect(treeKeyAction(open, 1, "End")).toEqual({ type: "focus", index: 2 });
  });

  it("opens and closes groups, and steps to the parent or first child", () => {
    expect(treeKeyAction(open, 0, "ArrowRight")).toEqual({ type: "focus", index: 1 });
    expect(treeKeyAction(open, 0, "ArrowLeft")).toEqual({ type: "expand", id: "r", expanded: false });
    expect(treeKeyAction(open, 1, "ArrowLeft")).toEqual({ type: "focus", index: 0 });
    const closed = rows(tasks).rows;
    expect(treeKeyAction(closed, 0, "ArrowRight")).toEqual({ type: "expand", id: "r", expanded: true });
  });

  it("opens the detail with Enter", () => {
    expect(treeKeyAction(open, 2, "Enter")).toEqual({ type: "open", id: "i" });
  });
});
