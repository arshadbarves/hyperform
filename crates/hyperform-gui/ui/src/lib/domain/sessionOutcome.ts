import type { TurnOutcome } from "../protocol/TurnOutcome";
import type { SessionActivity, UnreadMark } from "./sessions";

export interface UnreadSessionOutcome {
  label: string;
  tone: "neutral" | "verified" | "warn";
}

/** Sidebar dot for a turn that finished while the chat was in the background. */
export function unreadSessionOutcome(
  mark: UnreadMark | null | undefined,
  active: boolean,
  activity: SessionActivity | null,
): UnreadSessionOutcome | null {
  if (active || activity || !mark) return null;
  switch (mark.outcome.type) {
    case "failed":
      return { label: "Response failed", tone: "warn" };
    case "budgetExhausted":
      return { label: "Stopped · budget reached", tone: "warn" };
    case "interrupted":
      return { label: "Interrupted", tone: "warn" };
    case "cancelled":
      return { label: "Stopped", tone: "neutral" };
    default:
      break;
  }
  switch (mark.verification.status) {
    case "verified":
      return { label: "Finished · verified", tone: "verified" };
    case "failed":
      return { label: "Finished · checks failed", tone: "warn" };
    case "unverified":
      return { label: "Finished · unverified", tone: "neutral" };
    default:
      return { label: "Finished", tone: "neutral" };
  }
}

export interface SidebarMark {
  tone: "attention" | "working" | "ok" | "neutral" | "danger";
  label: string;
}

const MARK_TONE = { verified: "ok", warn: "attention", neutral: "neutral" } as const;

/**
 * The one status mark a chat row shows. The active chat shows none, since
 * the title bar already reports it; settled chats show a mark only when their
 * last response did not complete.
 */
export function sidebarMark(input: {
  active: boolean;
  activity: SessionActivity | null;
  unread: UnreadMark | null | undefined;
  lastOutcome: TurnOutcome | null;
}): SidebarMark | null {
  if (input.active) return null;
  if (input.activity === "approval") return { tone: "attention", label: "Needs you" };
  if (input.activity === "working") return { tone: "working", label: "Working" };
  const unread = unreadSessionOutcome(input.unread, false, null);
  if (unread) return { tone: MARK_TONE[unread.tone], label: unread.label };
  switch (input.lastOutcome?.type) {
    case "failed":
      return { tone: "danger", label: "Last response failed" };
    case "budgetExhausted":
      return { tone: "danger", label: "Stopped · budget reached" };
    case "interrupted":
      return { tone: "danger", label: "Interrupted" };
    default:
      return null;
  }
}
