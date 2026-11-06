import { describe, expect, it } from "vitest";
import {
  buildDiffTree,
  expandAncestors,
  filterDiffTree,
  flattenDiffTree,
} from "./diffTree";

describe("buildDiffTree", () => {
  it("nests folders and sorts dirs before files", () => {
    const tree = buildDiffTree([
      { path: "src/b.ts", status: "modified" },
      { path: "src/a/x.ts", status: "added" },
      { path: "z.md", status: "deleted" },
    ]);
    expect(tree.map((n) => n.name)).toEqual(["src", "z.md"]);
    const src = tree[0];
    expect(src?.kind).toBe("dir");
    if (src?.kind !== "dir") return;
    expect(src.children.map((n) => n.name)).toEqual(["a", "b.ts"]);
  });

  it("shows a chain of single folders as one row", () => {
    const tree = buildDiffTree([
      { path: "crates/app/src/a.rs", status: "modified" },
      { path: "crates/app/src/b.rs", status: "modified" },
    ]);
    expect(tree).toHaveLength(1);
    expect(tree[0]).toMatchObject({ kind: "dir", name: "crates/app/src", path: "crates/app/src" });
  });
});

describe("flattenDiffTree", () => {
  it("returns leaves in display order", () => {
    const tree = buildDiffTree([
      { path: "a/one.ts", status: "modified" },
      { path: "b.ts", status: "added" },
    ]);
    expect(flattenDiffTree(tree)).toEqual(["a/one.ts", "b.ts"]);
  });
});

describe("filterDiffTree", () => {
  it("drops folders with no matching leaves", () => {
    const tree = buildDiffTree([
      { path: "keep/hit.ts", status: "modified" },
      { path: "drop/miss.ts", status: "added" },
    ]);
    const filtered = filterDiffTree(tree, "hit");
    expect(flattenDiffTree(filtered)).toEqual(["keep/hit.ts"]);
  });
});

describe("expandAncestors", () => {
  it("opens every parent folder of the selection", () => {
    expect([...expandAncestors("a/b/c.ts")].sort()).toEqual(["a", "a/b"]);
  });
});
