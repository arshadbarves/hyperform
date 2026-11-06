import { describe, expect, it } from "vitest";
import { pairResults } from "./blocks";
import { workSection } from "./groups";
import { longChatSnapshot } from "./longChat";
import { buildTimeline } from "./turns";
import { openWindow } from "./window";

describe("longChatSnapshot", () => {
  const snap = longChatSnapshot(1000);
  const timeline = buildTimeline({ messages: snap.messages, turns: snap.turns });

  it("builds a finished chat of the asked length", () => {
    expect(timeline).toHaveLength(1000);
    expect(timeline.every((turn) => turn.user && turn.record && !turn.active)).toBe(true);
  });

  it("mixes plain answers, single calls, folded work and failures", () => {
    const results = pairResults(snap.messages);
    const sections = timeline.map((turn) => workSection(turn.items, true, (id) => results[id]?.isError === true));
    expect(sections.some((s) => s.work.length === 0)).toBe(true);
    expect(sections.some((s) => !s.fold && s.work.length > 0)).toBe(true);
    expect(sections.some((s) => s.fold && s.pinned.length === 0)).toBe(true);
    expect(sections.some((s) => s.pinned.length > 0)).toBe(true);
  });

  it("opens on a window of its newest turns", () => {
    const win = openWindow(timeline.length);
    expect(timeline.slice(win.start).map((turn) => turn.key)[0]).toBe(`u${win.start}`);
  });
});
