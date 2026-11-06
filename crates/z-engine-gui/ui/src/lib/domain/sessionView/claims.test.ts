import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import { info, snapshot, turnRecord } from "../testFixtures";
import { claimNote } from "../verification";
import { reduce } from "./reduce";
import { emptyView, type SessionView } from "./types";

const claimed: Event = { type: "completionClaimUnchecked", claim: { done: true, checks: true } };

function run(events: Event[], start: SessionView = emptyView("S1")): SessionView {
  return events.reduce((view, event, i) => reduce(view, event, 1000 + i), start);
}

describe("unchecked completion claims", () => {
  it("belong to the active turn and outlive its end and a snapshot", () => {
    const view = run([
      { type: "turnStarted", turnId: "t1", messageId: "u1" },
      claimed,
      { type: "turnFinished", turn: turnRecord({ verification: { status: "unverified", reason: "no checks" } }) },
      { type: "snapshot", snapshot: snapshot({ info: info() }) },
    ]);
    expect(view.claims).toEqual({ t1: { done: true, checks: true } });
  });

  it("are dropped without an active turn", () => {
    expect(run([claimed]).claims).toEqual({});
  });

  it("show only beside an unverified badge", () => {
    const unverified = { status: "unverified", reason: "no checks" } as const;
    expect(claimNote({ done: true, checks: true }, unverified)?.label).toBe("Claimed, not checked");
    expect(claimNote({ done: true, checks: false }, unverified)?.hint).toContain("work is done");
    expect(claimNote({ done: false, checks: true }, unverified)?.hint).toContain("tests or checks passed");
    expect(claimNote(undefined, unverified)).toBeNull();
    expect(claimNote({ done: true, checks: true }, { status: "failed", reason: "x" })).toBeNull();
    expect(claimNote({ done: true, checks: true }, { status: "verified", checks: [] })).toBeNull();
  });
});
