import type { CheckRecord } from "../protocol/CheckRecord";
import type { TurnOutcome } from "../protocol/TurnOutcome";
import type { TurnRecord } from "../protocol/TurnRecord";
import type { UncheckedClaim } from "../protocol/UncheckedClaim";
import type { VerificationOutcome } from "../protocol/VerificationOutcome";
import { fmtDuration } from "./format";

export type BadgeTone = "ok" | "warn" | "err" | "neutral";

export interface VerificationBadge {
  label: "Verified" | "Unverified" | "Failed" | "Not applicable";
  tone: BadgeTone;
  reason: string | null;
}

export function verificationBadge(outcome: VerificationOutcome): VerificationBadge {
  switch (outcome.status) {
    case "verified":
      return { label: "Verified", tone: "ok", reason: null };
    case "unverified":
      return { label: "Unverified", tone: "warn", reason: outcome.reason };
    case "failed":
      return { label: "Failed", tone: "err", reason: outcome.reason };
    default:
      return { label: "Not applicable", tone: "neutral", reason: "No files changed" };
  }
}

/** Only non-default endings are called out; a completed turn needs no label. */
export function outcomeNote(outcome: TurnOutcome): { label: string; tone: BadgeTone } | null {
  switch (outcome.type) {
    case "cancelled":
      return { label: "Cancelled", tone: "neutral" };
    case "failed":
      return { label: `Failed · ${outcome.message}`, tone: "err" };
    case "budgetExhausted":
      return { label: `Stopped · ${outcome.reason}`, tone: "warn" };
    case "interrupted":
      return { label: "Interrupted · the app closed mid-turn", tone: "warn" };
    default:
      return null;
  }
}

/**
 * A success claim no check backed, called out beside an `unverified` badge
 * only; any other outcome already says what the checks found.
 */
export function claimNote(
  claim: UncheckedClaim | undefined,
  outcome: VerificationOutcome,
): { label: string; hint: string } | null {
  if (!claim || outcome.status !== "unverified") return null;
  const what = claim.checks ? "says tests or checks passed" : "says the work is done";
  return { label: "Claimed, not checked", hint: `The final message ${what}, but no check confirmed it.` };
}

/**
 * Evidence behind a turn's badge: the cited records for `verified`, else the
 * checks that ran while the turn was open.
 */
export function turnEvidence(turn: TurnRecord, checks: CheckRecord[]): CheckRecord[] {
  if (turn.verification.status === "verified") {
    const ids = new Set(turn.verification.checks);
    return checks.filter((c) => ids.has(c.recordId));
  }
  return checks.filter((c) => c.startedAt >= turn.startedAt && c.startedAt <= turn.finishedAt);
}

export function checkStatus(record: CheckRecord): { label: string; tone: BadgeTone } {
  if (record.timedOut) return { label: "timed out", tone: "err" };
  if (record.passed) return { label: "passed", tone: "ok" };
  return { label: record.exitCode === null ? "failed" : `exit ${record.exitCode}`, tone: "err" };
}

/** `12 passed · 1 failed · 2 skipped · 3.2s` */
export function checkDetail(record: CheckRecord): string {
  const parts: string[] = [];
  if (record.tests) {
    const { passed, failed, skipped } = record.tests;
    parts.push(`${passed} passed`);
    if (failed) parts.push(`${failed} failed`);
    if (skipped) parts.push(`${skipped} skipped`);
  }
  const dur = fmtDuration(record.durationMs);
  if (dur) parts.push(dur);
  return parts.join(" · ");
}
