import { describe, expect, it } from "vitest";
import { followTotal, hasOlder, openWindow, reveal, sampleEvenly, showOlder, WINDOW_INITIAL, WINDOW_STEP } from "./window";

describe("turn window", () => {
  it("opens on the newest turns, or on all of a short chat", () => {
    expect(openWindow(1000)).toEqual({ start: 1000 - WINDOW_INITIAL, total: 1000 });
    expect(openWindow(12)).toEqual({ start: 0, total: 12 });
    expect(hasOlder(openWindow(12))).toBe(false);
  });

  it("reaches back a step at a time and stops at the first turn", () => {
    const win = showOlder(openWindow(1000));
    expect(win.start).toBe(1000 - WINDOW_INITIAL - WINDOW_STEP);
    expect(showOlder({ start: 5, total: 50 })).toEqual({ start: 0, total: 50 });
    const first = { start: 0, total: 50 };
    expect(showOlder(first)).toBe(first);
  });

  it("includes a turn you jump to, and leaves the window alone when it is already in", () => {
    const win = openWindow(1000);
    expect(reveal(win, 12).start).toBe(12);
    expect(reveal(win, 990)).toBe(win);
  });

  it("keeps its opening size while you follow the newest turn", () => {
    const win = followTotal(showOlder(openWindow(100)), 101, true);
    expect(win).toEqual({ start: 101 - WINDOW_INITIAL, total: 101 });
  });

  it("keeps its start while you read back, so nothing above you moves", () => {
    const win = followTotal(showOlder(openWindow(100)), 101, false);
    expect(win).toEqual({ start: 100 - WINDOW_INITIAL - WINDOW_STEP, total: 101 });
  });

  it("opens fresh when a chat's history arrives, and shows enough after a rewind", () => {
    expect(followTotal(openWindow(0), 1000, false)).toEqual(openWindow(1000));
    expect(followTotal({ start: 90, total: 100 }, 40, false)).toEqual({ start: 40 - WINDOW_INITIAL, total: 40 });
  });
});

describe("sampleEvenly", () => {
  it("keeps a short list as it is", () => {
    const list = [1, 2, 3];
    expect(sampleEvenly(list, 5)).toBe(list);
  });

  it("spreads picks over a long list and keeps both ends", () => {
    const list = Array.from({ length: 1000 }, (_, i) => i);
    const picked = sampleEvenly(list, 40);
    expect(picked).toHaveLength(40);
    expect(picked[0]).toBe(0);
    expect(picked.at(-1)).toBe(999);
    expect(new Set(picked).size).toBe(40);
  });
});
