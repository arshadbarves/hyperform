import { describe, expect, it } from "vitest";
import type { CompactionMarker } from "../../protocol/CompactionMarker";
import type { Event } from "../../protocol/Event";
import { snapshot, turnRecord } from "../testFixtures";
import { reduce } from "./reduce";
import { emptyView, type SessionView } from "./types";

const STARTED: Event = { type: "compactionStarted", trigger: "auto" };

const marker: CompactionMarker = {
  keepFrom: "u2",
  summary: "Earlier work",
  tokensBefore: 182_000,
  tokensAfter: 24_000,
  createdAt: 5,
};

function run(events: Event[], start: SessionView = emptyView("S1")): SessionView {
  return events.reduce((view, event, i) => reduce(view, event, 1000 + i), start);
}

describe("compaction", () => {
  it("starts out not compacting", () => {
    expect(emptyView("S1").compacting).toBe(false);
  });

  it("flags a running compaction until its marker arrives", () => {
    const running = run([STARTED]);
    expect(running.compacting).toBe(true);
    const done = reduce(running, { type: "compacted", marker }, 2000);
    expect(done.compacting).toBe(false);
    expect(done.compactions).toEqual([marker]);
  });

  it("stops when the turn ends or the session goes idle", () => {
    expect(run([STARTED, { type: "turnFinished", turn: turnRecord() }]).compacting).toBe(false);
    expect(run([STARTED, { type: "statusChanged", status: "idle" }]).compacting).toBe(false);
    expect(run([STARTED, { type: "statusChanged", status: "busy" }]).compacting).toBe(true);
  });

  it("stops when the turn fails or is cancelled mid-compaction", () => {
    const failed = turnRecord({ outcome: { type: "failed", message: "summary request failed" } });
    expect(run([STARTED, { type: "turnFinished", turn: failed }]).compacting).toBe(false);
    const cancelled = turnRecord({ outcome: { type: "cancelled" } });
    expect(run([STARTED, { type: "turnFinished", turn: cancelled }]).compacting).toBe(false);
  });

  it("clears a manual compaction that only ends in a notice once the session idles", () => {
    const manual: Event = { type: "compactionStarted", trigger: "manual" };
    const notice: Event = { type: "notice", level: "warn", text: "Compaction failed" };
    expect(run([manual, notice]).compacting).toBe(true);
    expect(run([manual, notice, { type: "statusChanged", status: "idle" }]).compacting).toBe(false);
  });

  it("gives up once the main agent streams again, not when a helper does", () => {
    expect(run([STARTED, { type: "assistantStarted", agentId: "main", messageId: "a1" }]).compacting).toBe(false);
    expect(run([STARTED, { type: "textDelta", agentId: "main", messageId: "a1", text: "x" }]).compacting).toBe(false);
    const helper = run([
      STARTED,
      { type: "assistantStarted", agentId: "agt_1", messageId: "s1" },
      { type: "thinkingDelta", agentId: "agt_1", messageId: "s1", text: "hmm" },
    ]);
    expect(helper.compacting).toBe(true);
  });

  it("survives a snapshot only while the session is busy", () => {
    const running = run([STARTED]);
    expect(reduce(running, { type: "snapshot", snapshot: snapshot({ status: "busy" }) }, 2).compacting).toBe(true);
    expect(reduce(running, { type: "snapshot", snapshot: snapshot() }, 2).compacting).toBe(false);
  });
});
