import { describe, expect, it } from "vitest";
import type { PromptInspect } from "./domain/requestInspect";
import { promptInsights } from "./promptInsights";

function snap(over: Partial<PromptInspect> = {}): PromptInspect {
  return {
    model: "test",
    sent: true,
    totalTokens: 4000,
    messages: [
      { role: "system", label: "System", content: "x".repeat(400), tokens: 100 },
      { role: "system", label: "Repo map", content: "m".repeat(800), tokens: 200 },
      { role: "user", label: "User", content: "hi", tokens: 10 },
      { role: "tool", label: "Tool result", content: "out".repeat(400), tokens: 300 },
    ],
    tools: [
      { name: "read_file", description: "d", schema: "{}", tokens: 400 },
      { name: "bash", description: "d", schema: "{}", tokens: 3990 },
    ],
    ...over,
  };
}

describe("promptInsights", () => {
  it("orders layers and flags the largest budget sinks", () => {
    const ins = promptInsights(snap());
    expect(ins.layers[0].label).toBe("System");
    expect(ins.layers.map((l) => l.order)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(ins.largest.name).toBe("bash");
    expect(ins.stack.some((s) => s.id === "tool def")).toBe(true);
    expect(ins.cacheableTokens).toBeGreaterThan(0);
    expect(ins.hints.some((h) => /tools/i.test(h))).toBe(true);
    expect(ins.stablePrefix).toBe("System");
    expect(ins.volatileTail).toContain("User");
  });

  it("notes when the snapshot is a preview rather than a sent request", () => {
    const ins = promptInsights(snap({ sent: false, tools: [], totalTokens: 310 }));
    expect(ins.hints.some((h) => /not been sent/i.test(h))).toBe(true);
  });
});
