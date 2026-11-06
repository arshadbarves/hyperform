import { describe, expect, it } from "vitest";
import { receiptPlan, turnFiles } from "./receipt";

const verified = { status: "verified" as const, checks: [] };
const none = { status: "notApplicable" as const };
const done = { type: "completed" as const };

describe("receiptPlan", () => {
  it("stays quiet: a verdict only when it says something, no numbers", () => {
    expect(receiptPlan("quiet", { verification: verified, outcome: done })).toEqual({
      badge: true,
      stats: false,
      usage: false,
      files: false,
      checksOpen: false,
    });
    expect(receiptPlan("quiet", { verification: none, outcome: done }).badge).toBe(false);
  });

  it("adds time, cost and changed files when compact", () => {
    expect(receiptPlan("compact", { verification: none, outcome: done })).toEqual({
      badge: false,
      stats: true,
      usage: false,
      files: true,
      checksOpen: false,
    });
  });

  it("shows everything when detailed, with the checks unfolded", () => {
    expect(receiptPlan("detailed", { verification: none, outcome: done })).toEqual({
      badge: true,
      stats: true,
      usage: true,
      files: true,
      checksOpen: true,
    });
  });

  it("always shows a failed verdict, whatever the setting", () => {
    const failed = { status: "failed" as const, reason: "tests" };
    expect(receiptPlan("quiet", { verification: failed, outcome: done }).badge).toBe(true);
    expect(receiptPlan("compact", { verification: failed, outcome: done }).checksOpen).toBe(false);
  });
});

describe("turnFiles", () => {
  it("lists the files a turn edited or wrote, once each, relative to the project", () => {
    const files = turnFiles(
      [
        { name: "Edit", input: { file_path: "/p/src/a.rs" }, ok: true },
        { name: "Write", input: { file_path: "/p/src/b.rs" }, ok: true },
        { name: "Edit", input: { file_path: "/p/src/a.rs" }, ok: true },
        { name: "Edit", input: { file_path: "/p/src/c.rs" }, ok: false },
        { name: "Read", input: { file_path: "/p/src/d.rs" }, ok: true },
      ],
      "/p",
    );
    expect(files).toEqual(["src/a.rs", "src/b.rs"]);
  });
});
