/**
 * CSS animation timing, so the pet's CSS motions can be drawn frame by
 * frame: the easings of motion.css, and keyframe tracks that ease between
 * each pair of stops the way a CSS animation does.
 */

/** Progress 0..1 to eased progress (a spring's may overshoot 1). */
export type Ease = (u: number) => number;

/** A CSS `cubic-bezier(x1, y1, x2, y2)`. */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): Ease {
  const curve = (a: number, b: number, s: number) => 3 * a * s * (1 - s) ** 2 + 3 * b * s * s * (1 - s) + s ** 3;
  return (u) => {
    if (u <= 0) return 0;
    if (u >= 1) return 1;
    let lo = 0;
    let hi = 1;
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2;
      if (curve(x1, x2, mid) < u) lo = mid;
      else hi = mid;
    }
    return curve(y1, y2, (lo + hi) / 2);
  };
}

/** The easings of motion.css; the springs use their cubic-bézier stand-ins. */
export const EASE = {
  linear: (u: number) => u,
  inOut: cubicBezier(0.65, 0, 0.35, 1),
  out: cubicBezier(0.16, 1, 0.3, 1),
  in: cubicBezier(0.4, 0, 1, 1),
  bouncy: cubicBezier(0.34, 1.36, 0.64, 1),
  gentle: cubicBezier(0.25, 0.8, 0.25, 1),
} satisfies Record<string, Ease>;

/** A keyframe: how far through the animation (0..1, a CSS percentage / 100) and the value there. */
export type Stop = readonly [at: number, value: number];

/**
 * A track's value at progress `u`: eased between the two stops around it,
 * as CSS eases each keyframe interval on its own. The stops run from 0 to 1.
 */
export function sampleTrack(stops: readonly Stop[], u: number, ease: Ease): number {
  const [firstAt, first] = stops[0];
  if (u <= firstAt) return first;
  for (let i = 1; i < stops.length; i++) {
    const [at, value] = stops[i];
    if (u > at) continue;
    const [prevAt, prev] = stops[i - 1];
    const span = at - prevAt;
    return span > 0 ? prev + (value - prev) * ease((u - prevAt) / span) : value;
  }
  return stops[stops.length - 1][1];
}

/**
 * How far through its current iteration an animation of `ms` per
 * iteration is (0..1), `tMs` after it started; null once its last of
 * `times` iterations has ended. Before it starts it is at 0.
 */
export function phase(tMs: number, ms: number, times = Infinity): number | null {
  if (!(tMs > 0) || !(ms > 0)) return 0;
  if (tMs >= ms * times) return null;
  return (tMs % ms) / ms;
}
