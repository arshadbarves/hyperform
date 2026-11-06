/** How often a view wants frames: every frame while it moves (at most 60 fps), or 30 fps for a mood loop. */
export type FrameRate = "move" | "loop";

/** A pet view on the frame loop. */
export interface FrameClient {
  /** Draws the view; `now` is the frame's timestamp in milliseconds. */
  draw(now: number): void;
  /**
   * The frames it wants after the frame it last drew, at `drawnAt`: a rate
   * while something was still playing then, null once it had come to rest.
   * Asked about that frame rather than the clock, so a motion always gets
   * a frame at or after its end, however late the frames come.
   */
  rate(drawnAt: number): FrameRate | null;
}

export interface FrameHandle {
  /** Something changed: draw once on the next frame, and ask `rate()` again. */
  invalidate(): void;
  /** Off the loop for good. */
  remove(): void;
}

interface Entry {
  client: FrameClient;
  dirty: boolean;
  last: number;
}

const INTERVAL_MS: Record<FrameRate, number> = { move: 1000 / 60, loop: 1000 / 30 };
/** A frame this early still counts as on time, so display jitter does not halve the rate. */
const SLACK_MS = 3;

function hidden(): boolean {
  return typeof document !== "undefined" && document.hidden;
}

/**
 * Calls `onChange` whenever the screen's pixel ratio changes (the window
 * moves to a display of another scale, or the page zooms), until the
 * returned stop is called. A resolution query matches one ratio only, so
 * it is asked again for the new ratio after each change.
 */
function watchScale(onChange: () => void): () => void {
  if (typeof matchMedia === "undefined") return () => {};
  let query: MediaQueryList | null = null;
  const arm = () => {
    query?.removeEventListener("change", changed);
    query = matchMedia(`(resolution: ${globalThis.devicePixelRatio || 1}dppx)`);
    query.addEventListener("change", changed);
  };
  const changed = () => {
    arm();
    onChange();
  };
  arm();
  return () => query?.removeEventListener("change", changed);
}

/**
 * One animation-frame loop for every pet view. It runs only while a view
 * wants frames or has a redraw pending, so an idle pet costs no frames.
 * Each view is drawn at most at its rate; a still one only when it is
 * invalidated, which is all a view asks for under Reduce Motion. The loop
 * stops while the document is hidden and picks up when it shows again,
 * and redraws every view when the screen's pixel ratio changes.
 */
export class FrameLoop {
  #entries = new Set<Entry>();
  #frame = 0;
  #ticking = false;
  #unwatch: (() => void) | null = null;

  /** Whether a frame is scheduled. */
  get running(): boolean {
    return this.#frame !== 0;
  }

  /** Puts a view on the loop; it draws on the next frame. */
  add(client: FrameClient): FrameHandle {
    const entry: Entry = { client, dirty: true, last: -Infinity };
    if (this.#entries.size === 0) {
      if (typeof document !== "undefined") document.addEventListener("visibilitychange", this.#wake);
      this.#unwatch = watchScale(this.#redrawAll);
    }
    this.#entries.add(entry);
    this.#wake();
    return {
      invalidate: () => {
        if (!this.#entries.has(entry)) return;
        entry.dirty = true;
        this.#wake();
      },
      remove: () => this.#remove(entry),
    };
  }

  #remove(entry: Entry) {
    if (!this.#entries.delete(entry) || this.#entries.size > 0) return;
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    if (typeof document !== "undefined") document.removeEventListener("visibilitychange", this.#wake);
    this.#unwatch?.();
    this.#unwatch = null;
  }

  #redrawAll = () => {
    for (const entry of this.#entries) entry.dirty = true;
    this.#wake();
  };

  #wanted(): boolean {
    for (const entry of this.#entries) if (entry.dirty || entry.client.rate(entry.last) !== null) return true;
    return false;
  }

  /** Schedules a frame if one is wanted; inside a frame, the frame decides at its end. */
  #wake = () => {
    if (this.#frame || this.#ticking || hidden() || !this.#wanted()) return;
    this.#frame = requestAnimationFrame(this.#tick);
  };

  #tick = (now: number) => {
    this.#frame = 0;
    if (hidden()) return;
    this.#ticking = true;
    try {
      for (const entry of this.#entries) {
        const rate = entry.dirty ? null : entry.client.rate(entry.last);
        if (!entry.dirty && (rate === null || now - entry.last < INTERVAL_MS[rate] - SLACK_MS)) continue;
        entry.dirty = false;
        entry.last = now;
        entry.client.draw(now);
      }
    } finally {
      this.#ticking = false;
    }
    this.#wake();
  };
}

/** The loop every pet view in the app shares. */
export const petFrames = new FrameLoop();
