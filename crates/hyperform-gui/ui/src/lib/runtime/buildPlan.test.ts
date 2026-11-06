import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Command } from "../protocol/Command";
import { envelope, info, snapshot } from "../domain/testFixtures";
import { workspaceStore } from "../workspaces";
import { CONTINUE_PROMPT } from "../domain/work/homeCard";
import { attachPlanHere, BUILD_PROMPT, buildPlan, keepPlanning, takePlanInNewChat } from "./planFlow";
import { sessions } from "./sessions.svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invoked = vi.mocked(invoke);

function sent(): [string, Command][] {
  return invoked.mock.calls
    .filter(([name]) => name === "send_command")
    .map(([, args]) => {
      const { sessionId, command } = args as { sessionId: string; command: Command };
      return [sessionId, command];
    });
}

function chat(id: string, planning = false) {
  sessions.apply(envelope({ type: "snapshot", snapshot: snapshot({ info: info({ planning }) }) }, 0, id));
  sessions.activate(id);
}

beforeEach(() => {
  invoked.mockReset();
  invoked.mockImplementation(async (name: string) => (name === "open_session" ? "NEW" : undefined));
  workspaceStore.setActive("/work/app");
});

describe("buildPlan", () => {
  it("answers the agent's plan when it waits for review, without a handoff to build here", async () => {
    chat("A");
    expect(await buildPlan({ planId: "P1", projectRoot: "/work/app", target: "here", requestId: "req_1" })).toBe("A");
    expect(sent()).toEqual([
      ["A", { type: "resolvePlan", requestId: "req_1", decision: { type: "approve", mode: null, editedPlan: null, handoff: false } }],
    ]);
  });

  it("approves an idle draft, leaves plan mode and tells the chat to start", async () => {
    chat("A", true);
    await buildPlan({ planId: "P1", projectRoot: "/work/app", target: "here", requestId: null });
    expect(sent().map(([, c]) => c)).toEqual([
      { type: "editWork", planId: "P1", edit: { type: "approve" } },
      { type: "setPlanning", planning: false },
      { type: "submit", text: BUILD_PROMPT, attachments: [] },
    ]);
  });

  it("hands off to a new chat in the plan's project that attaches the plan and starts", async () => {
    chat("A");
    expect(await buildPlan({ planId: "P1", projectRoot: "/work/other", target: "newChat", requestId: "req_1" })).toBe("NEW");
    expect(invoked).toHaveBeenCalledWith("open_session", { projectRoot: "/work/other", sessionId: null });
    expect(sent()).toEqual([
      ["A", { type: "resolvePlan", requestId: "req_1", decision: { type: "approve", mode: null, editedPlan: null, handoff: true } }],
      ["NEW", { type: "attachPlan", planId: "P1" }],
      ["NEW", { type: "setPlanning", planning: false }],
      ["NEW", { type: "submit", text: BUILD_PROMPT, attachments: [] }],
    ]);
  });

  it("approves parallel helpers right after the plan, before the chat starts", async () => {
    chat("A", true);
    await buildPlan({ planId: "P1", projectRoot: "/work/app", target: "here", requestId: null, parallel: true });
    expect(sent().map(([, c]) => c)).toEqual([
      { type: "editWork", planId: "P1", edit: { type: "approve" } },
      { type: "editWork", planId: "P1", edit: { type: "approveParallel" } },
      { type: "setPlanning", planning: false },
      { type: "submit", text: BUILD_PROMPT, attachments: [] },
    ]);
  });

  it("approves parallel helpers after answering the agent's plan", async () => {
    chat("A");
    expect(await buildPlan({ planId: "P1", projectRoot: "/work/app", target: "here", requestId: "req_1", parallel: true })).toBe("A");
    expect(sent().map(([, c]) => c.type)).toEqual(["resolvePlan", "editWork"]);
    expect(sent()[1][1]).toEqual({ type: "editWork", planId: "P1", edit: { type: "approveParallel" } });
  });

  it("stops when the approval fails", async () => {
    chat("A");
    invoked.mockRejectedValueOnce(new Error("gone"));
    expect(await buildPlan({ planId: "P1", projectRoot: "/work/app", target: "newChat", requestId: "req_1" })).toBeNull();
    expect(invoked).not.toHaveBeenCalledWith("open_session", expect.anything());
  });
});

describe("continuing a plan", () => {
  it("takes the plan in a new chat in its project and tells it to continue", async () => {
    chat("A");
    expect(await takePlanInNewChat("P1", "/work/other", CONTINUE_PROMPT)).toBe("NEW");
    expect(invoked).toHaveBeenCalledWith("open_session", { projectRoot: "/work/other", sessionId: null });
    expect(sent()).toEqual([
      ["NEW", { type: "attachPlan", planId: "P1" }],
      ["NEW", { type: "setPlanning", planning: false }],
      ["NEW", { type: "submit", text: CONTINUE_PROMPT, attachments: [] }],
    ]);
  });

  it("only attaches a plan it is not told to continue, such as a paused one", async () => {
    expect(await takePlanInNewChat("P1", "/work/app", null)).toBe("NEW");
    expect(sent()).toEqual([["NEW", { type: "attachPlan", planId: "P1" }]]);
  });

  it("attaches a plan to the open chat, or to a new one when none is open", async () => {
    chat("A");
    expect(await attachPlanHere("P2")).toBe("A");
    sessions.activate(null);
    expect(await attachPlanHere("P3")).toBe("NEW");
    expect(sent()).toEqual([
      ["A", { type: "attachPlan", planId: "P2" }],
      ["NEW", { type: "attachPlan", planId: "P3" }],
    ]);
  });
});

describe("keepPlanning", () => {
  it("revises the plan under review, or asks again in plan mode", async () => {
    chat("A");
    await keepPlanning("req_1", "Split the sync task");
    await keepPlanning(null, "Split the sync task");
    expect(sent().map(([, c]) => c)).toEqual([
      { type: "resolvePlan", requestId: "req_1", decision: { type: "revise", feedback: "Split the sync task" } },
      { type: "setPlanning", planning: true },
      { type: "submit", text: "Split the sync task", attachments: [] },
    ]);
  });
});
