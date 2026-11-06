import { describe, expect, it } from "vitest";
import type { AgentInfo } from "../protocol/AgentInfo";
import { emptyView } from "./sessionView/types";
import { agentInfo } from "./testFixtures";
import {
  clampPanelWidth,
  EMPTY_WATCH,
  isPanelTab,
  PANEL_DEFAULT_W,
  PANEL_MAX_W,
  PANEL_MIN_W,
  PANEL_TAB_SPECS,
  panelNudge,
  panelTabs,
  STAGE_MIN_W,
  stepTab,
  watchOf,
  type PanelWatch,
} from "./sidePanel";

const agent = (agentId: string, status: AgentInfo["status"]): AgentInfo => agentInfo({ agentId, status });

const watch = (over: Partial<PanelWatch> = {}): PanelWatch => ({ sessionId: "s1", plans: [], agents: [], ...over });

describe("panel tabs", () => {
  it("lists changes, plan, agents and context in order", () => {
    expect(panelTabs().map((t) => t.id)).toEqual(["changes", "plan", "agents", "context"]);
    expect(panelTabs().map((t) => t.label)).toEqual(["Changes", "Plan", "Agents", "Context"]);
  });

  it("keeps browser reserved and out of the strip", () => {
    expect(PANEL_TAB_SPECS.some((t) => t.id === "browser" && !t.available)).toBe(true);
    expect(panelTabs().some((t) => (t.id as string) === "browser")).toBe(false);
    expect(isPanelTab("browser")).toBe(false);
    expect(isPanelTab("plan")).toBe(true);
  });

  it("steps between shown tabs and wraps", () => {
    expect(stepTab("changes", 1)).toBe("plan");
    expect(stepTab("context", 1)).toBe("changes");
    expect(stepTab("changes", -1)).toBe("context");
  });
});

describe("clampPanelWidth", () => {
  it("keeps a width that fits", () => {
    expect(clampPanelWidth(500, 1400)).toBe(500);
  });

  it("never goes below the minimum or past the maximum", () => {
    expect(clampPanelWidth(100, 1400)).toBe(PANEL_MIN_W);
    expect(clampPanelWidth(5000)).toBe(PANEL_MAX_W);
  });

  it("leaves the chat its minimum beside the panel", () => {
    expect(clampPanelWidth(900, 1000)).toBe(1000 - STAGE_MIN_W);
  });

  it("keeps the panel minimum when the room is too small", () => {
    expect(clampPanelWidth(500, 500)).toBe(PANEL_MIN_W);
  });

  it("falls back to the default for a width that is not a number", () => {
    expect(clampPanelWidth(Number.NaN, 2000)).toBe(PANEL_DEFAULT_W);
  });
});

describe("watchOf", () => {
  it("sees pending plans and running helpers, not finished ones or the main agent", () => {
    const view = emptyView("s1");
    view.plans = { r1: { requestId: "r1", agentId: "main", plan: "Do it" } };
    view.agents = { main: agent("main", "running"), a1: agent("a1", "running"), a2: agent("a2", "completed") };
    expect(watchOf(view)).toEqual({ sessionId: "s1", plans: ["r1"], agents: ["a1"] });
  });

  it("is empty without a chat", () => {
    expect(watchOf(null)).toEqual(EMPTY_WATCH);
  });
});

describe("panelNudge", () => {
  it("opens the Plan tab when a plan becomes pending", () => {
    expect(panelNudge(watch(), watch({ plans: ["r1"] }), null)).toEqual({ open: "plan", badge: [] });
    expect(panelNudge(watch(), watch({ plans: ["r1"] }), "changes").open).toBe("plan");
  });

  it("does not reopen for a plan it has already seen", () => {
    expect(panelNudge(watch({ plans: ["r1"] }), watch({ plans: ["r1"] }), null).open).toBeNull();
  });

  it("badges Agents when a helper starts, without taking focus", () => {
    expect(panelNudge(watch(), watch({ agents: ["a1"] }), "changes")).toEqual({ open: null, badge: ["agents"] });
    expect(panelNudge(watch(), watch({ agents: ["a1"] }), null)).toEqual({ open: null, badge: ["agents"] });
  });

  it("needs no badge while Agents is showing", () => {
    expect(panelNudge(watch(), watch({ agents: ["a1"] }), "agents").badge).toEqual([]);
  });

  it("treats switching chats as no news", () => {
    const other = { sessionId: "s2", plans: ["r9"], agents: ["a9"] };
    expect(panelNudge(watch(), other, null)).toEqual({ open: null, badge: [] });
    expect(panelNudge(watch(), EMPTY_WATCH, null)).toEqual({ open: null, badge: [] });
  });
});
