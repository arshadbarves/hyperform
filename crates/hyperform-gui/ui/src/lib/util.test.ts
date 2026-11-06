import { describe, expect, it } from "vitest";
import { fmtCost, fmtTokens, relTime, shortModel } from "./util";

const NOW = new Date("2026-08-25T15:00:00").getTime();

describe("relTime", () => {
  it("buckets elapsed time", () => {
    expect(relTime(NOW - 30_000, NOW)).toBe("now");
    expect(relTime(NOW - 120_000, NOW)).toBe("2m");
    expect(relTime(NOW - 7_200_000, NOW)).toBe("2h");
    expect(relTime(NOW - 172_800_000, NOW)).toBe("2d");
  });
});

describe("formatting", () => {
  it("fmtTokens", () => {
    expect(fmtTokens(940)).toBe("940");
    expect(fmtTokens(12_345)).toBe("12k");
    expect(fmtTokens(1_234)).toBe("1.2k");
    expect(fmtTokens(1_234_567)).toBe("1.23M");
  });

  it("fmtCost", () => {
    expect(fmtCost(null)).toBe("–");
    expect(fmtCost(0)).toBe("$0.00");
    expect(fmtCost(0.0012)).toBe("$0.0012");
    expect(fmtCost(0.123)).toBe("$0.123");
    expect(fmtCost(1.234)).toBe("$1.23");
  });

  it("shortModel drops the provider prefix", () => {
    expect(shortModel("anthropic/claude-sonnet-4")).toBe("claude-sonnet-4");
    expect(shortModel("local")).toBe("local");
  });
});
