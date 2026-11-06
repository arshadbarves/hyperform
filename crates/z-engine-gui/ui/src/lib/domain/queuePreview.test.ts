import { describe, expect, it } from "vitest";
import { queuePreview, queueTitle, removeQueued, replaceQueued } from "./queuePreview";

describe("queuePreview", () => {
  it("uses the trimmed prompt text", () => {
    expect(queuePreview("  run the tests  ")).toBe("run the tests");
  });

  it("collapses newlines so the chip stays one line", () => {
    expect(queuePreview("first\n\nsecond\nthird")).toBe("first second third");
  });

  it("truncates long prompts with an ellipsis", () => {
    const label = queuePreview("x".repeat(80));
    expect(label).toBe(`${"x".repeat(47)}…`);
    expect(label.length).toBe(48);
  });

  it("keeps text that exactly fills the budget verbatim", () => {
    const text = "y".repeat(48);
    expect(queuePreview(text)).toBe(text);
  });

  it("falls back to a neutral label for blank items", () => {
    expect(queuePreview("   ")).toBe("Empty follow-up");
  });
});

describe("queueTitle", () => {
  it("shows the full untruncated prompt so the tooltip adds information", () => {
    const text = "z".repeat(120);
    expect(queueTitle(text)).toBe(text);
  });

  it("trims surrounding whitespace but keeps interior newlines", () => {
    expect(queueTitle("  deploy staging\n")).toBe("deploy staging");
    expect(queueTitle("first\nsecond")).toBe("first\nsecond");
    expect(queueTitle("\t")).toBe("Empty follow-up");
  });
});

describe("queue edits", () => {
  it("replaces or removes one queued message", () => {
    expect(replaceQueued(["a", "b"], 1, " c ")).toEqual(["a", "c"]);
    expect(replaceQueued(["a", "b"], 0, "  ")).toEqual(["b"]);
    expect(removeQueued(["a", "b", "c"], 1)).toEqual(["a", "c"]);
  });
});
