import { describe, expect, it } from "vitest";
import { groupPalette, paletteScore, rankPalette } from "./palette";

const item = (label: string, keywords = "", group = "Actions") => ({ label, keywords, group });

describe("paletteScore", () => {
  it("prefers the start of the name, then the start of a word, then anywhere", () => {
    const q = "rev";
    const start = paletteScore(q, item("Review changes"))!;
    const word = paletteScore(q, item("Open review"))!;
    const inside = paletteScore(q, item("Unrevealed"))!;
    const keyword = paletteScore(q, item("Diff", "review changes"))!;
    expect(start).toBeLessThan(word);
    expect(word).toBeLessThan(inside);
    expect(inside).toBeLessThan(keyword);
  });

  it("still finds letters typed in order, below real matches", () => {
    expect(paletteScore("nc", item("New chat"))).not.toBeNull();
    expect(paletteScore("nc", item("New chat"))!).toBeGreaterThan(paletteScore("new", item("New chat"))!);
    expect(paletteScore("zq", item("New chat"))).toBeNull();
  });

  it("does not match letters scattered from the middle of a word or across the name", () => {
    expect(paletteScore("cost", item("Allow private network", "localhost fetch"))).toBeNull();
    expect(paletteScore("cost", item("Run read-only shell commands without asking"))).toBeNull();
  });

  it("matches everything, in order, when the query is empty", () => {
    expect(paletteScore("  ", item("Anything"))).toBe(0);
  });
});

describe("rankPalette", () => {
  it("keeps matches only, best first, ties in their original order", () => {
    const list = [item("Settings"), item("New chat"), item("New chat in a worktree")];
    expect(rankPalette(list, "new").map((i) => i.label)).toEqual(["New chat", "New chat in a worktree"]);
    expect(rankPalette(list, "").map((i) => i.label)).toEqual(["Settings", "New chat", "New chat in a worktree"]);
  });
});

describe("groupPalette", () => {
  it("groups in the order groups first appear, numbering rows across groups", () => {
    const groups = groupPalette([item("a", "", "Chats"), item("b", "", "Actions"), item("c", "", "Chats")]);
    expect(groups.map((g) => [g.name, g.items.map((r) => [r.item.label, r.index])])).toEqual([
      ["Chats", [["a", 0], ["c", 2]]],
      ["Actions", [["b", 1]]],
    ]);
  });
});
