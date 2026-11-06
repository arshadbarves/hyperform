import type { AuditFinding } from "../../protocol/AuditFinding";
import type { PlanItem } from "../../protocol/PlanItem";
import type { Task } from "../../protocol/Task";
import type { WorkView } from "../sessionView/work";
import { needsRecheck } from "./criteria";
import { replanHeldTasks, replanWaitLine } from "./held";
import { blockKind, blockedText, isClosed } from "./status";

/**
 * What the plan waits on you for, one item each, for the Plan tab's Needs
 * you strip. Approvals and questions stay in the composer; the change set
 * after replanning stays in its footer, and the strip only notes the tasks
 * it holds back.
 */
export type NeedItem =
  /** Bringing a helper's change back conflicted; the tree is untouched and the worktree kept. */
  | { kind: "conflict"; key: string; task: Task; paths: string[]; worktree: string | null }
  /** A helper's change waits at its merge card (`merge = ask`). */
  | { kind: "apply"; key: string; task: Task; agentId: string | null }
  /** Tasks held until you accept the plan change; the other ready tasks keep running. */
  | { kind: "replan"; key: string; tasks: Task[] }
  /** Ended without a report, its change was discarded, or blocked for a reason only you clear. */
  | { kind: "retry"; key: string; task: Task; reason: string }
  /** Verifying, but a check did not run or its result is stale. */
  | { kind: "recheck"; key: string; task: Task }
  /** A requirement or invariant an agent proposed. */
  | { kind: "item"; key: string; item: PlanItem }
  | { kind: "task"; key: string; task: Task }
  /** An open high-severity audit finding; medium and low ones wait only in the findings list. */
  | { kind: "finding"; key: string; finding: AuditFinding };

export type NeedKind = NeedItem["kind"];

/** Most urgent first: stopped work, then finished work waiting, then what changes the plan, then the audit. */
const RANK: Record<NeedKind, number> = {
  conflict: 0,
  apply: 1,
  replan: 2,
  retry: 2,
  recheck: 2,
  item: 3,
  task: 4,
  finding: 5,
};

/** The findings that wait on you, as the kernel counts them: open and high severity. */
export function waitingFinding(finding: AuditFinding): boolean {
  return finding.status === "open" && finding.severity === "high";
}

function taskNeeds(task: Task, work: WorkView): NeedItem | null {
  if (isClosed(task.status)) return null;
  const integration = task.run.integration;
  const status = task.status;
  if (integration?.type === "conflict" || (status.type === "blocked" && blockKind(status.reason) === "conflict")) {
    return {
      kind: "conflict",
      key: `conflict:${task.id}`,
      task,
      paths: integration?.type === "conflict" ? integration.paths : [],
      worktree: integration?.type === "conflict" ? integration.worktree : null,
    };
  }
  if (integration?.type === "pendingApply" && status.type === "active") {
    return { kind: "apply", key: `apply:${task.id}`, task, agentId: task.run.dispatch?.agentId ?? null };
  }
  if (status.type === "blocked" && blockKind(status.reason) === "retry") {
    return { kind: "retry", key: `retry:${task.id}`, task, reason: blockedText(status.reason, work.tasks) };
  }
  if (needsRecheck(task, work.findings)) return { kind: "recheck", key: `recheck:${task.id}`, task };
  if (status.type === "proposed") return { kind: "task", key: `task:${task.id}`, task };
  return null;
}

function order(a: NeedItem, b: NeedItem): number {
  const rank = RANK[a.kind] - RANK[b.kind];
  if (rank !== 0) return rank;
  if ("task" in a && "task" in b) return a.task.updatedAt - b.task.updatedAt || a.task.order - b.task.order;
  return 0;
}

/** Every item, most urgent first; findings in the audit's own order. */
export function needsYouItems(work: WorkView): NeedItem[] {
  const items: NeedItem[] = [];
  const held = replanHeldTasks(work);
  if (held.length) items.push({ kind: "replan", key: "replan", tasks: held });
  for (const task of Object.values(work.tasks)) {
    const need = taskNeeds(task, work);
    if (need) items.push(need);
  }
  for (const item of [...work.plan.requirements, ...work.plan.invariants]) {
    if (item.status === "proposed") items.push({ kind: "item", key: `item:${item.id}`, item });
  }
  for (const finding of work.audit) {
    if (waitingFinding(finding)) items.push({ kind: "finding", key: `finding:${finding.id}`, finding });
  }
  return items.sort(order);
}

/**
 * The plan summary's `needsYou` for these items: the kernel counts each
 * one except the replan note, whose tasks wait on the change set instead.
 */
export function needsYouCount(items: readonly NeedItem[]): number {
  return items.filter((need) => need.kind !== "replan").length;
}

/** The strip's one-line headline per item. */
export function needLine(need: NeedItem): string {
  switch (need.kind) {
    case "conflict":
      return `Merge conflict in “${need.task.title}”`;
    case "apply":
      return `“${need.task.title}” is ready to apply`;
    case "replan":
      return replanWaitLine(need.tasks.length) ?? "";
    case "retry":
      return `“${need.task.title}” stopped: ${need.reason}`;
    case "recheck":
      return `“${need.task.title}” waits on checks that did not run or are stale`;
    case "item":
      return `Proposed ${need.item.kind}: ${need.item.text}`;
    case "task":
      return `Proposed task: ${need.task.title}`;
    case "finding":
      return need.finding.text;
  }
}

/** How many audit findings the strip shows; the rest are a count that opens the findings list. */
export const FINDINGS_SHOWN = 2;

/**
 * What the strip shows: every item the plan waits on is never folded;
 * audit findings past the first few become a count, since the findings
 * list is their home.
 */
export function needsStrip(items: readonly NeedItem[]): { shown: NeedItem[]; moreFindings: number } {
  const shown: NeedItem[] = [];
  let findings = 0;
  for (const need of items) {
    if (need.kind === "finding" && ++findings > FINDINGS_SHOWN) continue;
    shown.push(need);
  }
  return { shown, moreFindings: Math.max(0, findings - FINDINGS_SHOWN) };
}
