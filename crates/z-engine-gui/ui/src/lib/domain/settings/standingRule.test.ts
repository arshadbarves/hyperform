import { describe, expect, it } from "vitest";
import { appendRule, hasRule } from "./standingRule";

describe("standing rules", () => {
  it("append as one list item at the end", () => {
    expect(appendRule("", "Always use pnpm.")).toBe("- Always use pnpm.\n");
    expect(appendRule("# Notes\n\n- Be brief\n\n", "  Always  use pnpm ")).toBe(
      "# Notes\n\n- Be brief\n- Always use pnpm\n",
    );
  });

  it("are found again whatever the case, spacing or final period", () => {
    expect(hasRule("- always use  PNPM\n", "Always use pnpm.")).toBe(true);
    expect(hasRule("- Always use npm\n", "Always use pnpm")).toBe(false);
  });
});
