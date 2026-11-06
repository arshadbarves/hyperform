import type { FeatureMode } from "../../protocol/config/FeatureMode";
import type { ParallelPolicy } from "../../protocol/ParallelPolicy";
import type { PlanHeader } from "../../protocol/PlanHeader";
import type { WorkView } from "../sessionView/work";
import type { NotebookTab } from "./notebook";
import type { TaskTone } from "./status";

/** The feature flag that turns persistent work plans on. */
export const WORK_PLANS_FEATURE = "work_plans";

/**
 * Which Plan tab to show. `off`: today's plan document (the feature is off);
 * `empty`: on, no plan attached; then the attached plan's status. Paused,
 * done and abandoned plans use the building layout under a state banner;
 * done and abandoned ones read-only until you reopen them.
 */
export type PanelKind = "off" | "empty" | "draft" | "building" | "replanning" | "paused" | "done" | "abandoned";

export interface PlanPanel {
  kind: PanelKind;
  /** Draft or replanning: waiting for your review before work goes on. */
  reviewing: boolean;
  /** Done or abandoned: nothing to edit until you reopen it. */
  readOnly: boolean;
}

export function workPlansOn(experimental: Readonly<Record<string, FeatureMode>> | null | undefined): boolean {
  const mode = experimental?.[WORK_PLANS_FEATURE];
  return mode !== undefined && mode !== "off";
}

export function planPanel(input: { work: WorkView | null; enabled: boolean }): PlanPanel {
  const { work, enabled } = input;
  if (!work) return { kind: enabled ? "empty" : "off", reviewing: false, readOnly: false };
  const status = work.plan.status;
  const kind: PanelKind = status === "active" ? "building" : status;
  return {
    kind,
    reviewing: kind === "draft" || kind === "replanning",
    readOnly: kind === "done" || kind === "abandoned",
  };
}

/** Building, replanning (other ready tasks keep running) or paused (helpers are being stopped): the Now lanes show. */
export function showsLanes(kind: PanelKind): boolean {
  return kind === "building" || kind === "replanning" || kind === "paused";
}

/** The header's status word and its tone: amber when it waits on you, green while building. */
export function statusWord(kind: PanelKind, pending: boolean): { label: string; tone: TaskTone } {
  switch (kind) {
    case "draft":
      return pending ? { label: "Draft · ready for review", tone: "attention" } : { label: "Draft", tone: "neutral" };
    case "building":
      return { label: "Building", tone: "working" };
    case "replanning":
      return { label: "Replanning · review the changes", tone: "attention" };
    case "paused":
      return { label: "Paused", tone: "quiet" };
    case "done":
      return { label: "Done", tone: "ok" };
    case "abandoned":
      return { label: "Abandoned", tone: "quiet" };
    default:
      return { label: "", tone: "quiet" };
  }
}

function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** "4 requirements · 2 invariants · 9 tasks", leaving out what has none. */
export function planCounts(plan: PlanHeader, tasks: number): string {
  const requirements = plan.requirements.filter((r) => r.status !== "retired").length;
  const invariants = plan.invariants.filter((r) => r.status !== "retired").length;
  return [
    requirements && count(requirements, "requirement"),
    invariants && count(invariants, "invariant"),
    count(tasks, "task"),
  ]
    .filter(Boolean)
    .join(" · ");
}

export type PlanAction =
  | "pause"
  | "requestReplan"
  | "approveParallel"
  | "runOneAtATime"
  | "stop"
  | "abandon"
  | "switchPlan"
  | "detach";

const ACTION_LABEL: Record<PlanAction, string> = {
  pause: "Pause",
  requestReplan: "Request replanning",
  approveParallel: "Allow helpers in parallel",
  runOneAtATime: "Run one task at a time",
  stop: "Stop",
  abandon: "Abandon the plan",
  switchPlan: "Switch to another plan…",
  detach: "Detach from this chat",
};

export interface PlanActionContext {
  /** The plan's policy while `work_parallel` is effective, else null. */
  parallel?: ParallelPolicy | null;
  /** A task is still bound to an agent, so a paused plan has helpers left to stop. */
  bound?: boolean;
  /** The project has other plans to switch to. */
  canSwitch?: boolean;
}

function stateActions(kind: PanelKind, toggle: PlanAction[], bound: boolean): PlanAction[] {
  switch (kind) {
    case "building":
      return ["pause", "requestReplan", ...toggle, "stop", "abandon"];
    case "replanning":
      return ["pause", "stop", "abandon"];
    case "paused":
      return bound ? ["stop", "abandon"] : ["abandon"];
    case "draft":
      return ["abandon"];
    default:
      return [];
  }
}

/**
 * The header menu per state; Resume and Reopen sit in the state banner.
 * Building offers to allow helpers in parallel, or to go back to one task
 * at a time once allowed. Stop (pause and stop every helper) is offered
 * while something can still run; a done plan cannot be abandoned.
 */
export function planActions(kind: PanelKind, ctx: PlanActionContext = {}): { action: PlanAction; label: string }[] {
  if (kind === "off" || kind === "empty") return [];
  const { parallel = null, bound = false, canSwitch = false } = ctx;
  const toggle: PlanAction[] =
    parallel === null ? [] : parallel === "approved" ? ["runOneAtATime"] : ["approveParallel"];
  const actions: PlanAction[] = [
    ...stateActions(kind, toggle, bound),
    ...(canSwitch ? (["switchPlan"] as const) : []),
    "detach",
  ];
  return actions.map((action) => ({ action, label: ACTION_LABEL[action] }));
}

/** A task still bound to an agent: Stop releases it. */
export function hasBound(work: WorkView): boolean {
  return Object.values(work.tasks).some(
    (t) => t.owner !== null && (t.status.type === "active" || t.status.type === "verifying"),
  );
}

/** Above this width, or expanded over the stage, the tree and the detail sit side by side. */
export const WIDE_PANEL_W = 720;

export function wideLayout(width: number, expanded: boolean): boolean {
  return expanded || width > WIDE_PANEL_W;
}

export type PlanViewMode = "tasks" | "document";

/** What the Plan tab keeps per plan, so switching chats or plans returns to the same place. */
export interface PlanViewState {
  view: PlanViewMode;
  /** The task whose detail is open. */
  selected: string | null;
  /** Rows opened (true) or closed (false) by hand; the others follow the tree's rules. */
  expanded: Readonly<Record<string, boolean>>;
  notebookOpen: boolean;
  notebookTab: NotebookTab;
  /** A notebook entry to open, e.g. from a transcript trace's Why. */
  focusEntry: string | null;
  showReplaced: boolean;
  /** The audit's findings list, opened from the coverage line. */
  findings: boolean;
}

export function emptyPlanView(): PlanViewState {
  return {
    view: "tasks",
    selected: null,
    expanded: {},
    notebookOpen: false,
    notebookTab: "decisions",
    focusEntry: null,
    showReplaced: false,
    findings: false,
  };
}

/**
 * Which place of a plan the Plan tab's body scrolls: the tasks, the
 * document, or (drilled in when narrow) a task's detail or the findings.
 * Each keeps its own scroll position.
 */
export function scrollPlace(state: PlanViewState, wide: boolean): string {
  if (state.view === "document") return "document";
  if (!wide && state.selected) return `task:${state.selected}`;
  if (!wide && state.findings) return "findings";
  return "tasks";
}

/**
 * Esc steps back out of a task's detail, then out of the findings list
 * (a task opened from a finding returns to it); null when there is
 * nothing to step out of.
 */
export function stepBack(state: PlanViewState): PlanViewState | null {
  if (state.selected) return { ...state, selected: null };
  return state.findings ? { ...state, findings: false } : null;
}

/** The Back button of a task's detail, named for where `stepBack` goes. */
export function backLabel(state: PlanViewState): string {
  return state.findings ? "Back to findings" : "Back to tasks";
}
