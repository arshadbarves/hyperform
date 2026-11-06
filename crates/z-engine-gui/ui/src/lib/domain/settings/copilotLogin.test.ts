import { describe, expect, it } from "vitest";
import { codeDeadline, hasExpired, pollDelayMs, pollStep, POLL_MARGIN_MS } from "./copilotLogin";

const code = { userCode: "ABCD-1234", verificationUri: "https://github.com/login/device", deviceCode: "dc", interval: 5, expiresIn: 900 };

describe("copilot sign-in polling", () => {
  it("waits the interval plus a three second margin", () => {
    expect(POLL_MARGIN_MS).toBe(3000);
    expect(pollDelayMs(5)).toBe(8000);
    expect(pollDelayMs(0)).toBe(3000);
    expect(pollDelayMs(-1)).toBe(3000);
  });

  it("keeps the interval while pending and takes GitHub's on slow down", () => {
    expect(pollStep({ status: "pending" }, 5)).toEqual({ next: "wait", interval: 5 });
    expect(pollStep({ status: "slowDown", interval: 10 }, 5)).toEqual({ next: "wait", interval: 10 });
    expect(pollStep({ status: "slowDown", interval: 0 }, 5)).toEqual({ next: "wait", interval: 10 });
  });

  it("ends on done or failure", () => {
    expect(pollStep({ status: "done" }, 5)).toEqual({ next: "done" });
    expect(pollStep({ status: "failed", reason: "access denied" }, 5)).toEqual({ next: "failed", reason: "access denied" });
  });

  it("gives up once the code's lifetime has passed", () => {
    const deadline = codeDeadline(code, 1_000);
    expect(deadline).toBe(901_000);
    expect(hasExpired(900_999, deadline)).toBe(false);
    expect(hasExpired(901_000, deadline)).toBe(true);
  });
});
