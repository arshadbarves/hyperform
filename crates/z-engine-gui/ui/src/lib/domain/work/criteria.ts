import type { Criterion } from "../../protocol/Criterion";
import type { CriterionMethod } from "../../protocol/CriterionMethod";
import type { CriterionStatus } from "../../protocol/CriterionStatus";
import type { EvidenceRef } from "../../protocol/EvidenceRef";
import type { ReconcileFinding } from "../../protocol/ReconcileFinding";
import type { Task } from "../../protocol/Task";
import { evidenceLabel } from "./notebook";
import type { TaskTone } from "./status";

export interface EvidenceLine {
  key: string;
  text: string;
  /** A check's result; null for notes, diffs and links. */
  passed: boolean | null;
  at: number | null;
  /** The tree changed since this check ran (the latest reconciliation says so). */
  stale: boolean;
  /** Files a diff touched, to open in the Changes tab. */
  files: string[];
  url: string | null;
}

export interface CriterionLine {
  id: string;
  text: string;
  method: string;
  status: CriterionStatus;
  label: string;
  tone: TaskTone;
  evidence: EvidenceLine[];
  /** Only you waive a criterion, and only while it does not hold. */
  canWaive: boolean;
  /** A criterion you check yourself, waiting on you. */
  canConfirm: boolean;
}

const STATUS: Record<CriterionStatus, [string, TaskTone]> = {
  pending: ["Not met yet", "neutral"],
  met: ["Met", "ok"],
  failed: ["Failed", "danger"],
  waived: ["Waived", "quiet"],
};

export function methodLabel(method: CriterionMethod): string {
  switch (method.type) {
    case "check":
      return `Check · ${method.kind}`;
    case "inspection":
      return "Inspection";
    default:
      return "Your check";
  }
}

/** Check records the latest reconciliation found stale for this task. */
export function staleRecords(findings: readonly ReconcileFinding[], taskId: string): Set<string> {
  return new Set(findings.flatMap((f) => (f.type === "staleEvidence" && f.taskId === taskId ? f.records : [])));
}

function evidenceLine(ref: EvidenceRef, i: number, stale: Set<string>): EvidenceLine {
  const base = { key: `${ref.type}:${i}`, text: evidenceLabel(ref), passed: null, at: null, stale: false, files: [], url: null };
  switch (ref.type) {
    case "check":
      return { ...base, key: ref.recordId, passed: ref.passed, at: ref.at, stale: stale.has(ref.recordId) };
    case "diff":
      return { ...base, files: ref.files };
    case "external":
      return { ...base, url: ref.url };
    default:
      return base;
  }
}

export function criterionLine(criterion: Criterion, stale: Set<string>): CriterionLine {
  const [label, tone] = STATUS[criterion.status];
  const open = criterion.status === "pending" || criterion.status === "failed";
  return {
    id: criterion.id,
    text: criterion.text,
    method: methodLabel(criterion.method),
    status: criterion.status,
    label,
    tone,
    evidence: criterion.evidence.map((ref, i) => evidenceLine(ref, i, stale)),
    canWaive: open,
    canConfirm: criterion.method.type === "user" && open,
  };
}

/** A task's acceptance criteria with their evidence, stale marks and what you may do. */
export function criterionLines(task: Task, findings: readonly ReconcileFinding[]): CriterionLine[] {
  const stale = staleRecords(findings, task.id);
  return task.acceptance.map((c) => criterionLine(c, stale));
}

/**
 * A Verifying task its checks could not settle: a check criterion has no
 * result (its check did not run, or the project has no such check) or its
 * result is stale. You can run its checks again.
 */
export function needsRecheck(task: Task, findings: readonly ReconcileFinding[]): boolean {
  if (task.status.type !== "verifying") return false;
  const stale = staleRecords(findings, task.id);
  return task.acceptance.some(
    (c) =>
      c.method.type === "check" &&
      (c.status === "pending" || c.evidence.some((e) => e.type === "check" && stale.has(e.recordId))),
  );
}
