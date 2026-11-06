import { describe, expect, it } from "vitest";
import type { ParallelPolicy } from "../../protocol/ParallelPolicy";
import type { PlanStatus } from "../../protocol/PlanStatus";
import { header, task, work } from "./fixtures";
import {
  backLabel,
  emptyPlanView,
  hasBound,
  planActions,
  planCounts,
  planPanel,
  scrollPlace,
  showsLanes,
  statusWord,
  stepBack,
  wideLayout,
  workPlansOn,
  type PanelKind,
  type PlanViewState,
} from "./panel";

describe("plan panel state", () => {
  it("is Off without a plan while the feature is off, Empty while it is on", () => {
    expect(planPanel({ work: null, enabled: false }).kind).toBe("off");
    expect(planPanel({ work: null, enabled: true }).kind).toBe("empty");
  });

  it("follows the attached plan's status", () => {
    const kind = (status: PlanStatus) =>
      planPanel({ work: work([], { plan: header({ status }) }), enabled: false });
    expect(kind("draft")).toMatchObject({ kind: "draft", reviewing: true, readOnly: false });
    expect(kind("active")).toMatchObject({ kind: "building", reviewing: false, readOnly: false });
    expect(kind("replanning")).toMatchObject({ kind: "replanning", reviewing: true });
    expect(kind("paused")).toMatchObject({ kind: "paused", readOnly: false });
    expect(kind("done")).toMatchObject({ kind: "done", readOnly: true });
    expect(kind("abandoned")).toMatchObject({ kind: "abandoned", readOnly: true });
  });

  it("shows the Now lanes while work runs, keeps running beside a plan change, or is being stopped", () => {
    expect(["draft", "building", "replanning", "paused", "done"].map((k) => showsLanes(k as PanelKind))).toEqual([
      false,
      true,
      true,
      true,
      false,
    ]);
  });

  it("reads the feature flag from the effective settings", () => {
    expect(workPlansOn({ work_plans: "on" })).toBe(true);
    expect(workPlansOn({ work_plans: "off" })).toBe(false);
    expect(workPlansOn({})).toBe(false);
    expect(workPlansOn(null)).toBe(false);
  });

  it("says amber only when the plan waits on you", () => {
    expect(statusWord("draft", true)).toEqual({ label: "Draft · ready for review", tone: "attention" });
    expect(statusWord("draft", false).tone).toBe("neutral");
    expect(statusWord("building", false).tone).toBe("working");
    expect(statusWord("replanning", false).tone).toBe("attention");
  });

  it("counts requirements, invariants and tasks, leaving out what has none", () => {
    const item = (id: string, kind: "requirement" | "invariant", status: "active" | "retired" = "active") => ({
      id,
      kind,
      text: id,
      provenance: "user" as const,
      status,
      revision: 1,
    });
    const plan = header({
      requirements: [item("r1", "requirement"), item("r2", "requirement"), item("r3", "requirement", "retired")],
      invariants: [item("i1", "invariant")],
    });
    expect(planCounts(plan, 9)).toBe("2 requirements · 1 invariant · 9 tasks");
    expect(planCounts(header(), 1)).toBe("1 task");
  });

  it("lays out side by side above 720 px or when expanded", () => {
    expect(wideLayout(700, false)).toBe(false);
    expect(wideLayout(721, false)).toBe(true);
    expect(wideLayout(400, true)).toBe(true);
  });

  it("offers the header actions that fit the state", () => {
    const names = (kind: PanelKind, ctx = {}) => planActions(kind, ctx).map((a) => a.action);
    expect(names("building")).toEqual(["pause", "requestReplan", "stop", "abandon", "detach"]);
    expect(names("replanning")).toEqual(["pause", "stop", "abandon", "detach"]);
    expect(names("paused")).toEqual(["abandon", "detach"]);
    expect(names("paused", { bound: true })).toEqual(["stop", "abandon", "detach"]);
    expect(names("draft")).toEqual(["abandon", "detach"]);
    expect(names("done")).toEqual(["detach"]);
    expect(names("abandoned", { canSwitch: true })).toEqual(["switchPlan", "detach"]);
    expect(names("off", { canSwitch: true })).toEqual([]);
    expect(names("empty")).toEqual([]);
  });

  it("offers parallel helpers while building only when the feature is effective", () => {
    const names = (parallel: ParallelPolicy | null) => planActions("building", { parallel }).map((a) => a.action);
    expect(names(null)).not.toContain("approveParallel");
    expect(names("off")).toEqual(["pause", "requestReplan", "approveParallel", "stop", "abandon", "detach"]);
    expect(names("ask")).toContain("approveParallel");
    expect(names("approved")).toContain("runOneAtATime");
    expect(planActions("draft", { parallel: "ask" }).map((a) => a.action)).toEqual(["abandon", "detach"]);
  });

  it("knows when a task is still bound to an agent", () => {
    const owner = { sessionId: "S1", agentId: "agt_1" };
    expect(hasBound(work([task("a", { status: { type: "active" }, owner })]))).toBe(true);
    expect(hasBound(work([task("a", { status: { type: "pending" }, owner })]))).toBe(false);
    expect(hasBound(work([task("a", { status: { type: "active" } })]))).toBe(false);
  });

  it("keeps a scroll place per view, and per drill-in only when narrow", () => {
    const at = (over: Partial<PlanViewState>, wide = false) => scrollPlace({ ...emptyPlanView(), ...over }, wide);
    expect(at({})).toBe("tasks");
    expect(at({ view: "document", selected: "t1" })).toBe("document");
    expect(at({ selected: "t1" })).toBe("task:t1");
    expect(at({ selected: "t1" }, true)).toBe("tasks");
    expect(at({ findings: true })).toBe("findings");
  });

  it("steps back out of a task's detail, then out of the findings", () => {
    expect(stepBack({ ...emptyPlanView(), selected: "t1" })?.selected).toBeNull();
    expect(stepBack({ ...emptyPlanView(), selected: "t1", findings: true })).toMatchObject({ selected: null, findings: true });
    expect(stepBack({ ...emptyPlanView(), findings: true })?.findings).toBe(false);
    expect(stepBack(emptyPlanView())).toBeNull();
  });

  it("names the task detail's Back after where it returns", () => {
    expect(backLabel({ ...emptyPlanView(), selected: "t1" })).toBe("Back to tasks");
    expect(backLabel({ ...emptyPlanView(), selected: "t1", findings: true })).toBe("Back to findings");
  });
});
