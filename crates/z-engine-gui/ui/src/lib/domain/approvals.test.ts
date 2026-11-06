import { describe, expect, it } from "vitest";
import { approvalQuestion, previewLines } from "./approvals";
import { approval } from "./testFixtures";

describe("approvalQuestion", () => {
  it("asks about a command in plain words", () => {
    expect(approvalQuestion(approval())).toBe("Allow Bash to run cargo test?");
  });

  it("asks about a file change by its path", () => {
    const edit = approval({ tool: "Edit", title: "Edit src/a.rs", preview: { type: "diff", path: "src/a.rs", diff: "@@" } });
    expect(approvalQuestion(edit)).toBe("Allow Edit to change src/a.rs?");
  });

  it("turns any other title into a question", () => {
    const fetch = approval({ tool: "WebFetch", title: "Fetch example.com", preview: null });
    expect(approvalQuestion(fetch)).toBe("Fetch example.com?");
    expect(approvalQuestion(approval({ title: "Run it?", preview: null }))).toBe("Run it?");
  });
});

describe("previewLines", () => {
  it("counts lines so long previews can fold", () => {
    expect(previewLines("a\nb\nc\n")).toBe(3);
    expect(previewLines("")).toBe(0);
  });
});
