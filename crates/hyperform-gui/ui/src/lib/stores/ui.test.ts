import { beforeEach, describe, expect, it } from "vitest";
import { PANEL_MAX_W, PANEL_MIN_W } from "../domain/sidePanel";
import { ui } from "./ui.svelte";

describe("side panel state", () => {
  beforeEach(() => {
    ui.closePanel();
    ui.panelBadges = [];
  });

  it("opens changes on a file in a scope", () => {
    ui.openPanel("changes", "src/main.rs", "git");
    expect(ui.panelTab).toBe("changes");
    expect(ui.panel.open).toBe(true);
    expect(ui.diffFocus).toBe("src/main.rs");
    expect(ui.diffScope).toBe("git");
  });

  it("opens agents following one agent", () => {
    ui.openPanel("agents", "agent-7");
    expect(ui.panelTab).toBe("agents");
    expect(ui.workTab).toBe("agents");
    expect(ui.agentTranscript).toBe("agent-7");
  });

  it("opens the jobs list inside the Agents tab", () => {
    ui.openJobs();
    expect(ui.panelTab).toBe("agents");
    expect(ui.workTab).toBe("jobs");
  });

  it("opens plan and context", () => {
    ui.openPanel("plan");
    expect(ui.panelTab).toBe("plan");
    ui.openPanel("context");
    expect(ui.panelTab).toBe("context");
  });

  it("closes, remembers the tab and reopens docked", () => {
    ui.openPanel("agents", "agent-7");
    ui.setPanelExpanded(true);
    ui.closePanel();
    expect(ui.panelTab).toBeNull();
    expect(ui.panelExpanded).toBe(false);
    expect(ui.agentTranscript).toBeNull();
    ui.togglePanel();
    expect(ui.panelTab).toBe("agents");
    expect(ui.panel.expanded).toBe(false);
  });

  it("stays expanded while switching tabs", () => {
    ui.openPanel("changes");
    ui.setPanelExpanded(true);
    ui.openPanel("context");
    expect(ui.panelExpanded).toBe(true);
  });

  it("switches tabs from the strip without resetting them", () => {
    ui.openJobs();
    ui.showPanelTab("changes");
    ui.showPanelTab("agents");
    expect(ui.workTab).toBe("jobs");
    ui.badgePanel(["plan"]);
    ui.showPanelTab("plan");
    expect(ui.panelBadges).toEqual([]);
  });

  it("toggles a tab open and shut, switching when another shows", () => {
    ui.togglePanel("changes");
    expect(ui.panelTab).toBe("changes");
    ui.togglePanel("plan");
    expect(ui.panelTab).toBe("plan");
    ui.togglePanel("plan");
    expect(ui.panelTab).toBeNull();
  });

  it("only expands while open", () => {
    ui.setPanelExpanded(true);
    expect(ui.panelExpanded).toBe(false);
    ui.openPanel("changes");
    ui.setPanelExpanded(true);
    expect(ui.panelExpanded).toBe(true);
  });

  it("clamps the width", () => {
    ui.setPanelWidth(10);
    expect(ui.panel.width).toBe(PANEL_MIN_W);
    ui.setPanelWidth(99_999);
    expect(ui.panel.width).toBe(PANEL_MAX_W);
  });

  it("badges a tab that is not showing and clears it once shown", () => {
    ui.openPanel("changes");
    ui.badgePanel(["agents", "changes"]);
    expect(ui.panelBadges).toEqual(["agents"]);
    ui.openPanel("agents");
    expect(ui.panelBadges).toEqual([]);
  });
});

describe("plan view state", () => {
  it("keeps each plan's place apart, so switching chats or plans returns to it", () => {
    ui.plan.select("P1", "t3");
    ui.plan.setExpanded("P1", "t1", true);
    ui.plan.set("P1", { notebookTab: "changes" });
    ui.plan.select("P2", "x");
    expect(ui.plan.of("P1")).toMatchObject({ selected: "t3", expanded: { t1: true }, notebookTab: "changes" });
    expect(ui.plan.of("P2").selected).toBe("x");
    expect(ui.plan.of("P3").selected).toBeNull();
  });

  it("keeps a scroll offset per plan and place", () => {
    ui.plan.keepScroll("P1", "tasks", 240);
    ui.plan.keepScroll("P1", "document", 80);
    expect(ui.plan.scrollOf("P1", "tasks")).toBe(240);
    expect(ui.plan.scrollOf("P1", "document")).toBe(80);
    expect(ui.plan.scrollOf("P2", "tasks")).toBe(0);
  });

  it("opens the picker for one chat, and leaves it when a plan opens", () => {
    ui.openPlanPicker("S1");
    expect(ui.panelTab).toBe("plan");
    expect(ui.plan.pickingFor).toBe("S1");
    ui.openPlan("P1", { selected: "t1" });
    expect(ui.plan.pickingFor).toBeNull();
    expect(ui.plan.of("P1").selected).toBe("t1");
  });
});
