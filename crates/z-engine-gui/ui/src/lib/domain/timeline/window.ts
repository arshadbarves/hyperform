/**
 * Which turns a long chat renders. It opens on the newest turns and reaches
 * back a step at a time as you scroll up, or at once to a turn you jump to.
 * Pending approvals and questions are docked in the composer, so no card
 * waiting on you can fall outside the window.
 */

/** Turns rendered when a chat opens. */
export const WINDOW_INITIAL = 30;
/** Older turns added each time the top of the transcript comes into view. */
export const WINDOW_STEP = 20;
/**
 * Newest turns that render in full at once; the others in the window are
 * placeholders that fill in as they near the screen.
 */
export const WINDOW_EAGER = 6;

/** Render turns `[start, total)`. */
export interface TurnWindow {
  start: number;
  total: number;
}

export function openWindow(total: number): TurnWindow {
  return { start: Math.max(0, total - WINDOW_INITIAL), total };
}

/**
 * The transcript changed length. While you follow the newest turn the window
 * keeps its opening size; while you read back it keeps its start and grows,
 * so nothing above you moves. A shorter transcript (a rewind) still shows at
 * least the opening size.
 */
export function followTotal(win: TurnWindow, total: number, following: boolean): TurnWindow {
  if (total === win.total) return win;
  if (win.total === 0) return openWindow(total);
  const newest = Math.max(0, total - WINDOW_INITIAL);
  if (total < win.total) return { start: Math.min(win.start, newest), total };
  return { start: following ? Math.max(win.start, newest) : win.start, total };
}

export function hasOlder(win: TurnWindow): boolean {
  return win.start > 0;
}

/** Reach `step` turns further back. */
export function showOlder(win: TurnWindow, step = WINDOW_STEP): TurnWindow {
  return win.start === 0 ? win : { ...win, start: Math.max(0, win.start - step) };
}

/** Reach back far enough to include turn `index`, e.g. the prompt you jump to. */
export function reveal(win: TurnWindow, index: number): TurnWindow {
  return index >= 0 && index < win.start ? { ...win, start: index } : win;
}

/** At most `max` entries spread evenly over `list`; the first and the last always stay. */
export function sampleEvenly<T>(list: T[], max: number): T[] {
  if (list.length <= max) return list;
  if (max <= 1) return list.slice(-Math.max(0, max));
  return Array.from({ length: max }, (_, i) => list[Math.round((i * (list.length - 1)) / (max - 1))]);
}
