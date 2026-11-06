import { describe, expect, it } from "vitest";
import { parseMarkdown } from "./markdown";

describe("parseMarkdown", () => {
  it("parses GitHub-flavored markdown", () => {
    const tree = parseMarkdown("| a |\n| - |\n| 1 |");
    expect(JSON.stringify(tree)).toContain('"tagName":"table"');
  });

  it("parses finished text once", () => {
    const text = "Some **finished** reply.";
    expect(parseMarkdown(text)).toBe(parseMarkdown(text));
  });

  it("does not keep text that is still streaming", () => {
    const text = "Half a repl";
    expect(parseMarkdown(text, false)).not.toBe(parseMarkdown(text, false));
  });
});
