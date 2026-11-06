import { describe, expect, it } from "vitest";
import type { RouteInfo } from "../../protocol/RouteInfo";
import { routeChip } from "./routeChip";

const route = (patch: Partial<RouteInfo>): RouteInfo => ({
  agentId: "main",
  effort: null,
  model: null,
  reason: "simple task, confidence 0.93",
  ...patch,
});

describe("routeChip", () => {
  it("names the effort and the model the main agent's task was routed to", () => {
    expect(routeChip(route({ effort: "low", model: "fast-model" }))).toEqual({
      lead: "Routed",
      choice: "low effort · fast-model",
      reason: "simple task, confidence 0.93",
    });
    expect(routeChip(route({ effort: "high" })).choice).toBe("high effort");
  });

  it("says when a subagent was routed", () => {
    const chip = routeChip(route({ agentId: "a1", model: "fast-model" }));
    expect(chip.lead).toBe("Subagent routed");
    expect(chip.choice).toBe("fast-model");
  });
});
