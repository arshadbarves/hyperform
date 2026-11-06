import { EASE, type Ease } from "../domain/pet/keyframes";

/**
 * A set of numbers easing from one value to the next, the way the SVG
 * pet's CSS transitions morph its eyes, brows and cheeks. A new target
 * starts from wherever the last one had got to, so a change mid-way does
 * not jump.
 */
export interface Tween<T extends Record<keyof T, number>> {
  from: T;
  to: T;
  at: number;
  ms: number;
  ease: Ease;
}

export function still<T extends Record<keyof T, number>>(value: T): Tween<T> {
  return { from: value, to: value, at: -Infinity, ms: 0, ease: EASE.out };
}

/** Its value at `now`. */
export function valueAt<T extends Record<keyof T, number>>(tween: Tween<T>, now: number): T {
  const u = tween.ms > 0 ? Math.min(1, Math.max(0, (now - tween.at) / tween.ms)) : 1;
  if (u >= 1) return tween.to;
  const k = tween.ease(u);
  const out = { ...tween.to };
  for (const key of Object.keys(out) as (keyof T)[]) {
    const a = tween.from[key];
    out[key] = (a + (tween.to[key] - a) * k) as T[keyof T];
  }
  return out;
}

function same<T extends Record<keyof T, number>>(a: T, b: T): boolean {
  return (Object.keys(a) as (keyof T)[]).every((key) => Math.abs(a[key] - b[key]) < 1e-6);
}

/** Heads for `to` over `ms` from its value at `now`; the same target keeps its tween, `ms` 0 jumps. */
export function retarget<T extends Record<keyof T, number>>(tween: Tween<T>, to: T, now: number, ms: number, ease = EASE.out): Tween<T> {
  if (same(tween.to, to)) return tween;
  if (ms <= 0) return still(to);
  return { from: valueAt(tween, now), to, at: now, ms, ease };
}

/** Starts over from `from`, whatever it showed before (a part appearing). */
export function restart<T extends Record<keyof T, number>>(from: T, to: T, now: number, ms: number, ease = EASE.out): Tween<T> {
  return ms <= 0 ? still(to) : { from, to, at: now, ms, ease };
}

/** Whether it is still on its way at `now`. */
export function moving<T extends Record<keyof T, number>>(tween: Tween<T>, now: number): boolean {
  return tween.ms > 0 && now < tween.at + tween.ms && !same(tween.from, tween.to);
}
