import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import { snapshot } from "../testFixtures";
import { reduce } from "./reduce";
import { emptyView, type SessionView } from "./types";

function run(view: SessionView, ...events: Event[]): SessionView {
  return events.reduce((v, event, index) => reduce(v, event, index), view);
}

describe("decision advice for the GUI", () => {
  it("keeps each turn's tone and the newest urgency per item", () => {
    const view = run(
      emptyView("S1"),
      { type: "turnToneJudged", turnId: "t1", tone: "struggling" },
      { type: "urgencyScored", urgency: { key: "req_1", urgency: "low" } },
      { type: "urgencyScored", urgency: { key: "req_1", urgency: "high" } },
      { type: "urgencyScored", urgency: { key: "notice:disk full", urgency: "normal" } },
    );
    expect(view.turnTones).toEqual({ t1: "struggling" });
    expect(view.urgency).toEqual({ req_1: "high", "notice:disk full": "normal" });
  });

  it("survives a snapshot", () => {
    const before = run(
      emptyView("S1"),
      { type: "turnToneJudged", turnId: "t1", tone: "done_well" },
      { type: "urgencyScored", urgency: { key: "req_1", urgency: "high" } },
    );
    const after = run(before, { type: "snapshot", snapshot: snapshot() });
    expect(after.turnTones).toEqual(before.turnTones);
    expect(after.urgency).toEqual(before.urgency);
  });
});
