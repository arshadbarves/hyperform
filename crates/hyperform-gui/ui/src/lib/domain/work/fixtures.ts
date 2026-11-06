/** Work-plan builders for unit tests and the dev-only mock backend (lib/runtime/dev); never imported by app code. */
import type { AuditFinding } from "../../protocol/AuditFinding";
import type { Criterion } from "../../protocol/Criterion";
import type { Entry } from "../../protocol/Entry";
import type { EntryBody } from "../../protocol/EntryBody";
import type { PlanHeader } from "../../protocol/PlanHeader";
import type { PlanHealth } from "../../protocol/PlanHealth";
import type { PlanSummary } from "../../protocol/PlanSummary";
import type { Task } from "../../protocol/Task";
import type { WorkUpdate } from "../../protocol/WorkUpdate";
import type { WorkView } from "../sessionView/work";

export function header(over: Partial<PlanHeader> = {}): PlanHeader {
  return {
    planId: "P1",
    projectRoot: "/repo",
    goal: "Add offline sync support",
    document: "# Offline sync",
    status: "draft",
    depth: "standard",
    parallel: "off",
    revision: 1,
    requirements: [],
    invariants: [],
    createdAt: 1_000,
    updatedAt: 1_000,
    ...over,
  };
}

export function task(id: string, over: Partial<Task> = {}): Task {
  return {
    id,
    parent: null,
    title: `Task ${id}`,
    activeForm: `Doing ${id}`,
    description: "",
    kind: "implement",
    priority: "normal",
    order: 0,
    status: { type: "pending" },
    held: false,
    runAlone: false,
    dependsOn: [],
    requirements: [],
    scope: { files: [], symbols: [] },
    notes: { assumptions: [], constraints: [], risks: [], rollback: null },
    acceptance: [],
    steps: [],
    owner: null,
    result: null,
    resume: null,
    attempts: 0,
    run: { dispatch: null, integration: null, report: null, forgivenAttempts: 0, serial: false },
    provenance: "agent",
    createdAt: 1_000,
    updatedAt: 1_000,
    createdRevision: 1,
    revision: 1,
    ...over,
  };
}

export function criterion(id: string, over: Partial<Criterion> = {}): Criterion {
  return { id, text: `Criterion ${id}`, method: { type: "inspection" }, status: "pending", evidence: [], ...over };
}

export function entry(id: string, body: EntryBody, over: Partial<Entry> = {}): Entry {
  return {
    id,
    body,
    provenance: "agent",
    status: { type: "accepted" },
    tasks: [],
    files: [],
    createdAt: 2_000,
    createdRevision: 2,
    revision: 2,
    ...over,
  };
}

export function work(tasks: Task[] = [], over: Partial<WorkView> = {}): WorkView {
  return {
    plan: header(),
    tasks: Object.fromEntries(tasks.map((t) => [t.id, t])),
    entries: {},
    findings: [],
    audit: [],
    health: null,
    replanHeld: [],
    moves: [],
    ...over,
  };
}

export function finding(id: string, over: Partial<AuditFinding> = {}): AuditFinding {
  return {
    id,
    kind: "taskWithoutCriteria",
    severity: "medium",
    text: `Finding ${id}`,
    refs: [],
    suggestion: null,
    status: "open",
    source: "deterministic",
    createdRevision: 1,
    revision: 1,
    ...over,
  };
}

export function health(over: Partial<PlanHealth> = {}): PlanHealth {
  return {
    requirements: 0,
    requirementsCovered: 0,
    tasks: 0,
    tasksDone: 0,
    tasksWithoutCriteria: 0,
    blocked: 0,
    failed: 0,
    evidence: "good",
    evidenceStale: 0,
    openFindings: 0,
    reconcileFindings: 0,
    unmitigatedRisks: 0,
    ...over,
  };
}

export function update(over: Partial<WorkUpdate> = {}): WorkUpdate {
  const plan = over.plan ?? header();
  return {
    planId: plan.planId,
    revision: plan.revision,
    plan,
    tasks: [],
    entries: [],
    change: null,
    audit: [],
    findings: [],
    health: health(),
    replanHeld: [],
    ...over,
  };
}

export function summary(id: string, over: Partial<PlanSummary> = {}): PlanSummary {
  return {
    planId: id,
    projectRoot: "/repo",
    goal: `Goal ${id}`,
    status: "active",
    depth: "standard",
    revision: 3,
    tasksTotal: 5,
    tasksDone: 2,
    blocked: 0,
    needsYou: 0,
    running: 0,
    lastSessionId: null,
    attachedTo: null,
    createdAt: 1_000,
    updatedAt: 1_000,
    ...over,
  };
}
