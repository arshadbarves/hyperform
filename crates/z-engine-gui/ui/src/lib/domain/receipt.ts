import type { JsonValue } from "../protocol/serde_json/JsonValue";
import type { TaskReportView } from "../protocol/config/TaskReportView";
import type { TurnOutcome } from "../protocol/TurnOutcome";
import type { VerificationOutcome } from "../protocol/VerificationOutcome";
import { relPath, str } from "./tools/toolInput";
import { toolMeta } from "./tools/toolMeta";

/** What the one-line turn receipt shows, from `ui.task_report_view`. */
export interface ReceiptPlan {
  /** The verification verdict. */
  badge: boolean;
  /** Duration and cost. */
  stats: boolean;
  /** Token counts. */
  usage: boolean;
  /** Chips for the files the turn changed. */
  files: boolean;
  /** The check evidence starts unfolded. */
  checksOpen: boolean;
}

/**
 * Quiet shows a verdict only when it says something (verified, unverified,
 * failed); compact adds time, cost and changed files; detailed shows
 * everything, "not applicable" included, with the checks unfolded.
 */
export function receiptPlan(
  view: TaskReportView,
  record: { verification: VerificationOutcome; outcome: TurnOutcome },
): ReceiptPlan {
  const meaningful = record.verification.status !== "notApplicable";
  switch (view) {
    case "detailed":
      return { badge: true, stats: true, usage: true, files: true, checksOpen: true };
    case "compact":
      return { badge: meaningful, stats: true, usage: false, files: true, checksOpen: false };
    default:
      return { badge: meaningful, stats: false, usage: false, files: false, checksOpen: false };
  }
}

/** Files a turn edited or wrote successfully, once each, relative to the project. */
export function turnFiles(calls: { name: string; input: JsonValue; ok: boolean }[], root: string | null): string[] {
  const seen = new Set<string>();
  for (const call of calls) {
    const family = toolMeta(call.name).family;
    if (!call.ok || (family !== "edit" && family !== "write")) continue;
    const path = str(call.input, "file_path", "notebook_path", "path");
    if (path) seen.add(relPath(path, root));
  }
  return [...seen];
}
