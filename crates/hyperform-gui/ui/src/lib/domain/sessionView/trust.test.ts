import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import { applyLocalEvent, dismissTrustRequest, emptySessionsState } from "../sessions";
import { info, snapshot } from "../testFixtures";
import { reduce } from "./reduce";
import { emptyView } from "./types";

const request: Event = { type: "trustRequired", projectRoot: "/repo", defines: ["hooks", "MCP servers"] };

describe("trust request", () => {
  it("is stored from trustRequired and cleared by the next snapshot", () => {
    const pending = reduce(emptyView("S1"), request, 1);
    expect(pending.trustRequest).toEqual({ projectRoot: "/repo", defines: ["hooks", "MCP servers"] });
    const reloaded = reduce(pending, { type: "snapshot", snapshot: snapshot({ info: info() }) }, 2);
    expect(reloaded.trustRequest).toBeNull();
  });

  it("is dismissed locally when the user answers", () => {
    const state = applyLocalEvent(emptySessionsState(), "S1", request, 1);
    expect(state.views.S1.trustRequest).not.toBeNull();
    const dismissed = dismissTrustRequest(state, "S1");
    expect(dismissed.views.S1.trustRequest).toBeNull();
    expect(dismissTrustRequest(dismissed, "S1")).toBe(dismissed);
    expect(dismissTrustRequest(dismissed, "missing")).toBe(dismissed);
  });
});
