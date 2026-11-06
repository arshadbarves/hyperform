import type { CopilotDeviceCode, CopilotLoginPoll } from "../../commands/settings";
import type { ChoiceOption } from "./options";

/**
 * GitHub Copilot sign-in with GitHub's device flow: how long to wait between
 * polls, what each poll means, and when the code has expired.
 */

/** Added to every wait so a poll never arrives before GitHub's interval has passed. */
export const POLL_MARGIN_MS = 3000;
/** Added to the interval when GitHub asks to slow down without saying by how much. */
const SLOW_DOWN_STEP_S = 5;

export const EXPIRED_REASON = "The code expired before you approved it. Sign in again for a new code.";

export type CopilotHost = "github" | "enterprise";

export const COPILOT_HOST_OPTIONS: readonly ChoiceOption<CopilotHost>[] = [
  { value: "github", label: "GitHub.com", description: "Sign in with your account on github.com." },
  { value: "enterprise", label: "Enterprise", description: "Sign in on your company's GitHub Enterprise domain." },
];

/** What the sign-in shows. */
export type CopilotLoginState =
  | { phase: "idle" }
  | { phase: "starting" }
  | { phase: "waiting"; userCode: string; verificationUri: string }
  | { phase: "done" }
  | { phase: "failed"; reason: string };

/** What follows one poll: another wait (with the interval to use), or the end. */
export type PollStep = { next: "wait"; interval: number } | { next: "done" } | { next: "failed"; reason: string };

/** Milliseconds before the next poll, for an interval in seconds. */
export function pollDelayMs(intervalSeconds: number): number {
  return Math.max(0, intervalSeconds) * 1000 + POLL_MARGIN_MS;
}

/** Pending keeps the interval; slow down takes the one GitHub returns. */
export function pollStep(poll: CopilotLoginPoll, interval: number): PollStep {
  switch (poll.status) {
    case "pending":
      return { next: "wait", interval };
    case "slowDown":
      return { next: "wait", interval: poll.interval > 0 ? poll.interval : interval + SLOW_DOWN_STEP_S };
    case "done":
      return { next: "done" };
    case "failed":
      return { next: "failed", reason: poll.reason };
  }
}

/** When the device code stops working, in epoch milliseconds. */
export function codeDeadline(code: CopilotDeviceCode, startedAt: number): number {
  return startedAt + code.expiresIn * 1000;
}

export function hasExpired(now: number, deadline: number): boolean {
  return now >= deadline;
}
