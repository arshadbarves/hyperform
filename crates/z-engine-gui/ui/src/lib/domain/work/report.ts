import type { EvidenceRef } from "../../protocol/EvidenceRef";
import type { Integration } from "../../protocol/Integration";
import type { Task } from "../../protocol/Task";
import { plural } from "../format";
import type { TaskTone } from "./status";

/** A labelled list from a report; empty lists are left out. */
export interface ReportList {
  label: string;
  items: string[];
}

export interface ObservedTest {
  label: string;
  passed: boolean | null;
}

/**
 * A task's Result: what the kernel observed first (files from the worktree
 * diff, the run's check records), then what the helper says, labelled as
 * its claims, then what it learned. A run that read web content marks
 * what it learned as from the web, unconfirmed.
 */
export interface ResultView {
  summary: string | null;
  /** The helper ended without a report; the task is blocked until you retry it. */
  unreported: boolean;
  /** Changed files from the diff; null when the engine could not read it. */
  files: string[] | null;
  tests: ObservedTest[];
  claims: ReportList[];
  learned: ReportList[];
  advice: ReportList[];
  confidence: string | null;
  fromWeb: boolean;
}

function list(label: string, items: readonly string[]): ReportList[] {
  const kept = items.map((s) => s.trim()).filter(Boolean);
  return kept.length ? [{ label, items: kept }] : [];
}

function test(ref: EvidenceRef): ObservedTest {
  switch (ref.type) {
    case "check":
      return { label: ref.label || ref.kind || "check", passed: ref.passed };
    case "diff":
      return { label: ref.summary || "diff", passed: null };
    case "note":
      return { label: ref.text, passed: null };
    case "external":
      return { label: ref.title || ref.url, passed: null };
  }
}

/** Null while the task has neither a report nor a result. */
export function resultView(task: Task): ResultView | null {
  const report = task.run.report;
  if (!report) {
    if (!task.result) return null;
    return {
      summary: task.result,
      unreported: false,
      files: [],
      tests: [],
      claims: [],
      learned: [],
      advice: [],
      confidence: null,
      fromWeb: false,
    };
  }
  return {
    summary: report.summary.trim() || task.result,
    unreported: report.source === "unreported",
    files: report.diffKnown ? report.filesChanged : null,
    tests: report.testsRun.map(test),
    claims: [
      ...list("Files", report.claimed.filesChanged),
      ...list("Tests", report.claimed.testsRun),
      ...list("Symbols", report.symbolsChanged),
    ],
    learned: [
      ...list("Discoveries", report.discoveries),
      ...list("Decisions", report.decisions),
      ...list("Experiments", report.experiments),
    ],
    advice: [...list("Blockers", report.blockers), ...list("Follow-ups", report.followUps), ...list("Risks", report.risks)],
    confidence: report.confidence,
    fromWeb: report.provenance === "research",
  };
}

/** How the helper's worktree change came back, in a line. */
export function integrationLine(integration: Integration | null): { text: string; tone: TaskTone } | null {
  if (!integration) return null;
  switch (integration.type) {
    case "pendingApply":
      return { text: "The helper's change waits for you to apply it", tone: "attention" };
    case "applied": {
      const n = integration.files.length;
      return { text: n ? `Applied to the project · ${plural(n, "file")}` : "Applied to the project", tone: "ok" };
    }
    case "conflict":
      return {
        text: `Applying it conflicts in ${integration.paths.join(", ")}. Your files were left as they were; the worktree is kept.`,
        tone: "attention",
      };
    case "discarded":
      return { text: "The helper's change was discarded", tone: "quiet" };
  }
}

/**
 * "Attempt 2 of 3" for a task a helper ran: the starts that count against
 * `work.max_attempts` (a retry by you forgives the earlier ones).
 */
export function attemptLine(task: Task, maxAttempts: number | null): string | null {
  if (!task.run.dispatch) return null;
  const used = task.attempts - task.run.forgivenAttempts;
  if (used <= 0) return null;
  return maxAttempts ? `Attempt ${used} of ${maxAttempts}` : `Attempt ${used}`;
}
