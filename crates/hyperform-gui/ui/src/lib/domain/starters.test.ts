import { describe, expect, it } from "vitest";
import { startersFor } from "./starters";

describe("startersFor", () => {
  it("offers three general starters and keeps the rest for later", () => {
    const { top, more } = startersFor({ changed: 0, hasInstructions: true });
    expect(top.map((s) => s.id)).toEqual(["explain", "bug", "tests"]);
    expect(more.map((s) => s.id)).toEqual(["cleanup"]);
  });

  it("puts a review first when the project has uncommitted changes", () => {
    const { top } = startersFor({ changed: 4, hasInstructions: true });
    expect(top[0]).toMatchObject({ id: "review", title: "Review my 4 uncommitted changes" });
    expect(top).toHaveLength(3);
  });

  it("suggests writing an AGENTS.md only when the project has none", () => {
    expect(startersFor({ changed: 0, hasInstructions: false }).top[0].id).toBe("agents-md");
    expect(startersFor({ changed: 0, hasInstructions: null }).top.map((s) => s.id)).not.toContain("agents-md");
  });

  it("keeps every starter a full prompt, never an empty draft", () => {
    const { top, more } = startersFor({ changed: 1, hasInstructions: false });
    for (const s of [...top, ...more]) expect(s.prompt.length).toBeGreaterThan(30);
  });
});
