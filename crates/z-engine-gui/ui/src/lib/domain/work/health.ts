import type { AuditFinding } from "../../protocol/AuditFinding";
import type { AuditKind } from "../../protocol/AuditKind";
import type { AuditSeverity } from "../../protocol/AuditSeverity";
import type { AuditStatus } from "../../protocol/AuditStatus";
import type { HealthLevel } from "../../protocol/HealthLevel";
import type { PlanHeader } from "../../protocol/PlanHeader";
import type { PlanHealth } from "../../protocol/PlanHealth";
import { plural } from "../format";
import { waitingFinding } from "./needsYou";
import type { TaskMap, TaskTone } from "./status";

const LEVEL: Record<HealthLevel, string> = { good: "Good", fair: "Fair", poor: "Poor" };

/**
 * The header's coverage line: "4 of 5 requirements covered · evidence Fair
 * · 2 findings". Counts and levels only, never a percentage. Evidence is
 * left out until a task is done (there is nothing to back yet). Amber
 * while a finding waits on you (an open high one); medium and low ones
 * are counted quietly.
 */
export function coverageLine(health: PlanHealth, audit: readonly AuditFinding[]): { text: string; tone: TaskTone } {
  const parts = [
    health.requirements > 0 && `${health.requirementsCovered} of ${plural(health.requirements, "requirement")} covered`,
    health.tasksDone > 0 && `evidence ${LEVEL[health.evidence]}`,
    health.openFindings > 0 && plural(health.openFindings, "finding"),
  ].filter((part): part is string => Boolean(part));
  return {
    text: parts.length ? parts.join(" · ") : "No open findings",
    tone: audit.some(waitingFinding) ? "attention" : "quiet",
  };
}

export const SEVERITY_LABEL: Record<AuditSeverity, string> = { high: "High", medium: "Medium", low: "Low" };
const SEVERITIES: AuditSeverity[] = ["high", "medium", "low"];

const KIND: Record<AuditKind, string> = {
  uncoveredRequirement: "Requirement not covered",
  criterionWithoutMethod: "Criterion cannot be checked",
  taskWithoutCriteria: "No criteria",
  riskyPathWithoutTest: "Risky path without a test",
  riskWithoutMitigation: "Risk without a mitigation",
  overlap: "Overlap",
  duplicate: "Duplicate",
  modelReview: "Review",
};

export function findingKind(kind: AuditKind): string {
  return KIND[kind];
}

/** A settled finding's state, as a quiet tag; open ones have none. */
export function findingStatus(status: AuditStatus): string | null {
  return status === "added" ? "added to the plan" : status === "open" ? null : status;
}

export interface FindingGroups {
  /** Open findings by severity, highest first; empty groups left out. */
  open: { severity: AuditSeverity; label: string; findings: AuditFinding[] }[];
  /** Added to the plan, for the agent to address. */
  added: AuditFinding[];
  /** Dismissed by you or resolved by a later audit; behind a disclosure. */
  settled: AuditFinding[];
}

export function findingGroups(audit: readonly AuditFinding[]): FindingGroups {
  return {
    open: SEVERITIES.map((severity) => ({
      severity,
      label: SEVERITY_LABEL[severity],
      findings: audit.filter((f) => f.status === "open" && f.severity === severity),
    })).filter((group) => group.findings.length > 0),
    added: audit.filter((f) => f.status === "added"),
    settled: audit.filter((f) => f.status === "dismissed" || f.status === "resolved"),
  };
}

/** What a finding is about, as links: tasks open their detail; requirements and files are plain. */
export interface FindingRef {
  key: string;
  label: string;
  taskId: string | null;
}

export function findingRefs(finding: AuditFinding, tasks: TaskMap, plan: PlanHeader): FindingRef[] {
  const items = [...plan.requirements, ...plan.invariants];
  const seen = new Set<string>();
  const out: FindingRef[] = [];
  for (const ref of finding.refs) {
    const next: FindingRef =
      ref.type === "task" || ref.type === "criterion"
        ? { key: `task:${ref.taskId}`, label: tasks[ref.taskId]?.title ?? ref.taskId, taskId: ref.taskId }
        : ref.type === "item"
          ? { key: `item:${ref.itemId}`, label: items.find((i) => i.id === ref.itemId)?.text ?? ref.itemId, taskId: null }
          : { key: `file:${ref.path}`, label: ref.path, taskId: null };
    if (seen.has(next.key)) continue;
    seen.add(next.key);
    out.push(next);
  }
  return out;
}
