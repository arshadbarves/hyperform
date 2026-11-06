import { describe, expect, it } from "vitest";
import { hasUnseen, recentActivity } from "./activity";
import { emptyView } from "./sessionView/types";

function source() {
  return {
    ...emptyView("S1"),
    notices: [{ id: 1, level: "info" as const, text: "Repo map ready", at: 10 }],
    hooks: [
      { id: 2, hookEvent: "PreToolUse", command: "lint.sh", blocked: true, message: "lint failed", at: 30 },
      { id: 3, hookEvent: "PostToolUse", command: "fmt.sh", blocked: false, message: null, at: 40 },
    ],
    errors: [{ id: 4, message: "Provider returned 500", afterMessageId: null, at: 20 }],
  };
}

describe("recentActivity", () => {
  it("merges notices, blocked hooks and errors, newest first", () => {
    expect(recentActivity(source()).map((i) => [i.tone, i.text])).toEqual([
      ["warn", "Hook blocked · PreToolUse: lint failed"],
      ["error", "Provider returned 500"],
      ["info", "Repo map ready"],
    ]);
  });

  it("limits the list and tolerates a missing view", () => {
    expect(recentActivity(source(), 1)).toHaveLength(1);
    expect(recentActivity(null)).toEqual([]);
  });
});

describe("hasUnseen", () => {
  it("counts only warnings and errors after the last look", () => {
    const items = recentActivity(source());
    expect(hasUnseen(items, 0)).toBe(true);
    expect(hasUnseen(items, 30)).toBe(false);
    expect(hasUnseen(items.filter((i) => i.tone === "info"), 0)).toBe(false);
  });
});
