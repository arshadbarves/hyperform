import { describe, expect, it } from "vitest";
import { markdownChunks } from "./markdownChunks";

describe("markdownChunks", () => {
  it("splits top-level blocks at blank lines and joins back to the text", () => {
    const text = "# Title\n\nFirst paragraph.\nStill first.\n\n```rust\nfn main() {}\n```\n\nLast.";
    const chunks = markdownChunks(text);
    expect(chunks).toEqual(["# Title\n", "First paragraph.\nStill first.\n", "```rust\nfn main() {}\n```\n", "Last."]);
    expect(chunks.join("\n")).toBe(text);
  });

  it("keeps blank lines inside a code fence, closed only by a matching fence", () => {
    const text = "````md\n```\n\ninner\n```\n\n````\n\nafter";
    expect(markdownChunks(text)).toEqual(["````md\n```\n\ninner\n```\n\n````\n", "after"]);
    expect(markdownChunks("~~~\na\n\nb\n~~~")).toHaveLength(1);
  });

  it("never splits before a line that may continue the block above", () => {
    expect(markdownChunks("- one\n\n- two\n\n  more of two")).toHaveLength(1);
    expect(markdownChunks("1. one\n\n2. two")).toHaveLength(1);
    expect(markdownChunks("para\n\n    code\n\n> quote")).toHaveLength(1);
  });

  it("keeps text with reference definitions whole", () => {
    expect(markdownChunks("See [the docs][1].\n\nMore.\n\n[1]: https://example.com")).toHaveLength(1);
    expect(markdownChunks("Note[^a].\n\nMore.\n\n[^a]: A footnote.")).toHaveLength(1);
  });

  it("leaves earlier blocks unchanged while a reply streams", () => {
    const full = "Intro.\n\n```ts\nconst a = 1;\n\nconst b = 2;\n```\n\nOutro here.";
    const settled = markdownChunks(full).slice(0, -1);
    for (let end = full.indexOf("Outro") + 1; end <= full.length; end++) {
      expect(markdownChunks(full.slice(0, end)).slice(0, -1)).toEqual(settled);
    }
  });
});
