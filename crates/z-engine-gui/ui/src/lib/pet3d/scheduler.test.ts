import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FrameLoop, type FrameClient, type FrameHandle, type FrameRate } from "./scheduler";

let queue = new Map<number, FrameRequestCallback>();
let nextId = 0;
let page = Object.assign(new EventTarget(), { hidden: false });

beforeEach(() => {
  queue = new Map();
  page = Object.assign(new EventTarget(), { hidden: false });
  vi.stubGlobal("document", page);
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    queue.set(++nextId, cb);
    return nextId;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => queue.delete(id));
});

afterEach(() => vi.unstubAllGlobals());

/** Runs the frames requested so far, at `now`. */
function frame(now: number) {
  const due = [...queue.values()];
  queue = new Map();
  for (const cb of due) cb(now);
}

/** A display refreshing every `step` ms, for `count` frames from `from`. */
function display(from: number, step: number, count: number) {
  for (let i = 0; i < count; i++) frame(from + i * step);
}

function view(rate: FrameRate | null = null) {
  const drawn: number[] = [];
  const client: FrameClient & { wants: FrameRate | null } = {
    wants: rate,
    draw: (now) => drawn.push(now),
    rate: () => client.wants,
  };
  return { client, drawn };
}

function setHidden(hidden: boolean) {
  page.hidden = hidden;
  page.dispatchEvent(new Event("visibilitychange"));
}

describe("FrameLoop", () => {
  it("draws a new view once, then stops: a still pet costs no frames", () => {
    const loop = new FrameLoop();
    const { client, drawn } = view();
    loop.add(client);
    expect(loop.running).toBe(true);
    display(0, 16, 5);
    expect(drawn).toEqual([0]);
    expect(loop.running).toBe(false);
    expect(queue.size).toBe(0);
  });

  it("folds invalidations before a frame into one redraw", () => {
    const loop = new FrameLoop();
    const { client, drawn } = view();
    const handle = loop.add(client);
    frame(0);
    handle.invalidate();
    handle.invalidate();
    handle.invalidate();
    expect(queue.size).toBe(1);
    display(16, 16, 3);
    expect(drawn).toEqual([0, 16]);
    expect(loop.running).toBe(false);
  });

  it("draws a moving view every frame at 60 Hz, and at most 60 fps on a 120 Hz display", () => {
    const at60 = new FrameLoop();
    const a = view("move");
    at60.add(a.client);
    display(0, 1000 / 60, 60);
    expect(a.drawn).toHaveLength(60);

    const at120 = new FrameLoop();
    const b = view("move");
    at120.add(b.client);
    display(0, 1000 / 120, 120);
    expect(b.drawn).toHaveLength(60);
  });

  it("caps a mood loop at 30 fps", () => {
    for (const hz of [60, 120, 144]) {
      const loop = new FrameLoop();
      const { client, drawn } = view("loop");
      loop.add(client);
      display(0, 1000 / hz, hz);
      expect(drawn.length).toBeGreaterThanOrEqual(28);
      expect(drawn.length).toBeLessThanOrEqual(30);
    }
  });

  it("stops once the view is still again, and wakes on the next invalidation", () => {
    const loop = new FrameLoop();
    const { client, drawn } = view("move");
    const handle = loop.add(client);
    display(0, 16, 3);
    client.wants = null;
    display(48, 16, 3);
    expect(drawn).toEqual([0, 16, 32]);
    expect(loop.running).toBe(false);

    client.wants = "loop";
    handle.invalidate();
    display(100, 16, 5);
    expect(drawn).toEqual([0, 16, 32, 100, 132, 164]);
    expect(loop.running).toBe(true);
  });

  it("draws a timed motion through to its end, even after a slow frame", () => {
    const loop = new FrameLoop();
    const drawn: number[] = [];
    loop.add({ draw: (now) => drawn.push(now), rate: (at) => (at < 100 ? "move" : null) });
    display(0, 30, 6);
    expect(drawn).toEqual([0, 30, 60, 90, 120]);
    expect(loop.running).toBe(false);

    const slow = new FrameLoop();
    const late: number[] = [];
    slow.add({ draw: (now) => late.push(now), rate: (at) => (at < 100 ? "move" : null) });
    frame(0);
    frame(400);
    frame(416);
    expect(late).toEqual([0, 400]);
    expect(slow.running).toBe(false);
  });

  it("keeps each view to its own rate on the one loop", () => {
    const loop = new FrameLoop();
    const moving = view("move");
    const looping = view("loop");
    const still = view();
    loop.add(moving.client);
    loop.add(looping.client);
    loop.add(still.client);
    display(0, 1000 / 60, 60);
    expect(moving.drawn).toHaveLength(60);
    expect(looping.drawn).toHaveLength(30);
    expect(still.drawn).toEqual([0]);
    expect(queue.size).toBe(1);
  });

  it("pauses while the document is hidden and picks up when it shows", () => {
    const loop = new FrameLoop();
    const { client, drawn } = view("move");
    loop.add(client);
    display(0, 16, 2);
    setHidden(true);
    display(32, 16, 3);
    expect(drawn).toEqual([0, 16]);
    expect(loop.running).toBe(false);

    setHidden(false);
    expect(loop.running).toBe(true);
    frame(500);
    expect(drawn).toEqual([0, 16, 500]);
  });

  it("does not start a hidden view until the document shows", () => {
    page.hidden = true;
    const loop = new FrameLoop();
    const { client, drawn } = view();
    loop.add(client);
    expect(loop.running).toBe(false);
    setHidden(false);
    frame(10);
    expect(drawn).toEqual([10]);
  });

  it("schedules one frame when a view invalidates while it draws", () => {
    const loop = new FrameLoop();
    let draws = 0;
    const handle: FrameHandle = loop.add({
      draw: () => {
        draws += 1;
        if (draws === 1) handle.invalidate();
      },
      rate: () => null,
    });
    frame(0);
    expect(queue.size).toBe(1);
    display(16, 16, 3);
    expect(draws).toBe(2);
  });

  it("stops and stops listening once the last view is removed", () => {
    const loop = new FrameLoop();
    const { client, drawn } = view("move");
    const handle = loop.add(client);
    frame(0);
    handle.remove();
    expect(loop.running).toBe(false);
    expect(queue.size).toBe(0);
    handle.invalidate();
    setHidden(true);
    setHidden(false);
    expect(queue.size).toBe(0);
    expect(drawn).toEqual([0]);
  });

  it("redraws every still view when the screen's scale changes, and keeps watching at the new scale", () => {
    const queries: (EventTarget & { media: string })[] = [];
    vi.stubGlobal("devicePixelRatio", 1);
    vi.stubGlobal("matchMedia", (media: string) => {
      const query = Object.assign(new EventTarget(), { media });
      queries.push(query);
      return query;
    });
    const rescale = (ratio: number) => {
      vi.stubGlobal("devicePixelRatio", ratio);
      queries.at(-1)?.dispatchEvent(new Event("change"));
    };
    const loop = new FrameLoop();
    const one = view();
    const two = view();
    const handles = [loop.add(one.client), loop.add(two.client)];
    frame(0);
    expect(loop.running).toBe(false);
    expect(queries.map((q) => q.media)).toEqual(["(resolution: 1dppx)"]);

    rescale(2);
    expect(queries.at(-1)?.media).toBe("(resolution: 2dppx)");
    display(16, 16, 3);
    rescale(1.5);
    frame(100);
    expect(one.drawn).toEqual([0, 16, 100]);
    expect(two.drawn).toEqual([0, 16, 100]);

    for (const handle of handles) handle.remove();
    rescale(1);
    expect(queue.size).toBe(0);
    expect(queries).toHaveLength(3);
  });
});
