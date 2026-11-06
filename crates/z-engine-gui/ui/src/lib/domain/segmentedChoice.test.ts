import { describe, expect, it } from "vitest";
import { nextSegmentIndex } from "./segmentedChoice";

describe("segmented choice navigation", () => {
  it.each([
    ["ArrowLeft", 0, 2],
    ["ArrowUp", 0, 2],
    ["ArrowRight", 2, 0],
    ["ArrowDown", 2, 0],
    ["Home", 1, 0],
    ["End", 1, 2],
  ] as const)("maps %s from index %i to index %i", (key, current, expected) => {
    expect(nextSegmentIndex(key, current, 3)).toBe(expected);
  });

  it("ignores unrelated keys and empty option lists", () => {
    expect(nextSegmentIndex("Enter", 1, 3)).toBeNull();
    expect(nextSegmentIndex("ArrowDown", 0, 0)).toBeNull();
  });
});
