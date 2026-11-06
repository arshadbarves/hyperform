import { describe, expect, it } from "vitest";
import type { GitDiffRow } from "../diffParse";
import { foldContext, langForPath, splitRows, summarizeFiles } from "./diffRows";

const ctx = (n: number): GitDiffRow => ({ kind: "ctx", oldNo: n, newNo: n, text: `line ${n}` });
const add = (n: number, text = "new"): GitDiffRow => ({ kind: "add", oldNo: null, newNo: n, text });
const del = (n: number, text = "old"): GitDiffRow => ({ kind: "del", oldNo: n, newNo: null, text });

describe("foldContext", () => {
  it("folds a long unchanged stretch between changes, keeping three lines each side", () => {
    const rows = [add(1), ...Array.from({ length: 20 }, (_, i) => ctx(i + 2)), add(22)];
    const out = foldContext(rows);
    const fold = out.find((r) => r.kind === "fold");
    expect(fold).toEqual({ kind: "fold", from: 4, count: 14 });
    expect(out.filter((r) => r.kind === "ctx")).toHaveLength(6);
  });

  it("keeps only the lines next to a change at the start or end of the file", () => {
    const rows = [...Array.from({ length: 12 }, (_, i) => ctx(i + 1)), add(13)];
    const out = foldContext(rows);
    expect(out[0]).toEqual({ kind: "fold", from: 0, count: 9 });
    expect(out.filter((r) => r.kind === "ctx")).toHaveLength(3);
  });

  it("leaves short stretches alone", () => {
    const rows = [add(1), ctx(2), ctx(3), ctx(4), ctx(5), ctx(6), ctx(7), ctx(8), add(9)];
    expect(foldContext(rows)).toEqual(rows);
  });

  it("does not fold what is already open", () => {
    const rows = [add(1), ...Array.from({ length: 20 }, (_, i) => ctx(i + 2)), add(22)];
    expect(foldContext(rows, new Set([4])).some((r) => r.kind === "fold")).toBe(false);
  });
});

describe("splitRows", () => {
  it("pairs removed lines with the added lines that replace them", () => {
    const out = splitRows([ctx(1), del(2, "a"), del(3, "b"), add(2, "A"), ctx(4)]);
    expect(out.map((r) => (r.kind === "pair" ? [r.left?.text ?? null, r.right?.text ?? null] : r.kind))).toEqual([
      ["line 1", "line 1"],
      ["a", "A"],
      ["b", null],
      ["line 4", "line 4"],
    ]);
  });

  it("puts a pure addition on the right only", () => {
    const out = splitRows([add(1, "x")]);
    expect(out).toEqual([{ kind: "pair", key: "p0", left: null, right: add(1, "x") }]);
  });
});

describe("langForPath", () => {
  it("maps extensions to a highlighter language", () => {
    expect(langForPath("src/main.rs")).toBe("rust");
    expect(langForPath("ui/App.svelte")).toBe("xml");
    expect(langForPath("Cargo.toml")).toBe("ini");
    expect(langForPath("notes.txt")).toBeNull();
  });
});

describe("summarizeFiles", () => {
  it("counts what changed across the files", () => {
    const s = summarizeFiles([
      { status: "added", added: 3, deleted: 0 },
      { status: "modified", added: 1, deleted: 2 },
      { status: "deleted", added: 0, deleted: 5 },
    ]);
    expect(s).toEqual({ files: 3, created: 1, updated: 1, deleted: 1, added: 4, removed: 7 });
  });
});
