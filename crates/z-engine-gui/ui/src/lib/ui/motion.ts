import { cubicOut } from "svelte/easing";
import type { TransitionConfig } from "svelte/transition";
import { springs, type SpringPreset } from "./springs";

/**
 * Options for `Spring` from `svelte/motion` (its own per-frame units, not the
 * physical springs in `springs.ts`): snappy for controls, smooth for size,
 * bouncy for the pet and droplets, gentle for long travel.
 */
export const SPRING = {
  snappy: { stiffness: 0.2, damping: 0.62 },
  smooth: { stiffness: 0.14, damping: 0.86 },
  bouncy: { stiffness: 0.16, damping: 0.42 },
  gentle: { stiffness: 0.07, damping: 0.95 },
} as const;

/** How long an exit plays when no stylesheet says otherwise (`--dur-exit`). */
export const EXIT_MS = 180;
const REDUCED_MS = 120;
/** Stands in for `linear()` where the engine lacks it (`--ease-spring-*` fallbacks in motion.css). */
const SPRING_FALLBACK = "cubic-bezier(0.2, 0.8, 0.2, 1)";

export function prefersReducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Parses a CSS time ("180ms", "1.6s") into milliseconds; null when it is not one. */
export function parseCssTime(value: string): number | null {
  const match = /^\s*(-?\d*\.?\d+)\s*(ms|s)\s*$/.exec(value);
  if (!match) return null;
  const amount = Number(match[1]);
  return match[2] === "s" ? amount * 1000 : amount;
}

/** A motion token from `motion.css` in milliseconds, or `fallback` where no stylesheet is loaded. */
export function motionMs(token: `--${string}`, fallback: number): number {
  if (typeof document === "undefined" || typeof getComputedStyle !== "function") return fallback;
  return parseCssTime(getComputedStyle(document.documentElement).getPropertyValue(token)) ?? fallback;
}

/** A spring preset as an easing string for the Web Animations API (`element.animate`). */
export function springEasingCss(preset: SpringPreset): string {
  const supported = typeof CSS !== "undefined" && CSS.supports?.("transition-timing-function", "linear(0, 1)");
  return supported ? springs[preset].easing : SPRING_FALLBACK;
}

type Direction = { direction?: "in" | "out" | "both" };
type DirectedTransition = TransitionConfig | ((options?: Direction) => TransitionConfig);

/** Runs `pick` once the direction is known: at once for `in:` / `out:`, per run for `transition:`. */
function directed(options: Direction | undefined, pick: (out: boolean) => TransitionConfig): DirectedTransition {
  if (options?.direction === "in" || options?.direction === "out") return pick(options.direction === "out");
  return (run) => pick(run?.direction === "out");
}

function fadeOnly(delay = 0): TransitionConfig {
  return { delay, duration: REDUCED_MS, css: (t) => `opacity: ${t}` };
}

type AppearParams = { delay?: number; scale?: number; y?: number };

/**
 * Appearing: content fades up into place on the smooth spring; leaving, it
 * fades out quickly. Transform and opacity only; fades under Reduce Motion.
 */
export function appear(_node: Element, { delay = 0, scale = 0.98, y = 6 }: AppearParams = {}, options?: Direction) {
  return directed(options, (out) => {
    if (prefersReducedMotion()) return fadeOnly(out ? 0 : delay);
    if (out) return { duration: EXIT_MS, easing: cubicOut, css: (t) => `opacity: ${t}` };
    return {
      delay,
      duration: springs.smooth.duration,
      easing: springs.smooth.at,
      css: (t, u) => `opacity: ${Math.min(1, t)}; scale: ${1 - u * (1 - scale)}; translate: 0 ${(u * y).toFixed(2)}px;`,
    };
  });
}

/**
 * A sheet (a full-window page): it settles from 98% with a fade on the smooth
 * spring and drops back a touch as it leaves. Fades under Reduce Motion.
 */
export function sheet(_node: Element, _params: Record<string, never> = {}, options?: Direction) {
  return directed(options, (out) => {
    if (prefersReducedMotion()) return fadeOnly();
    if (out) return { duration: EXIT_MS, easing: cubicOut, css: (t, u) => `opacity: ${t}; scale: ${1 - u * 0.015};` };
    return {
      duration: springs.smooth.duration,
      easing: springs.smooth.at,
      css: (t, u) => `opacity: ${Math.min(1, t)}; scale: ${1 - u * 0.02};`,
    };
  });
}

type SplitParams = { from?: number };

/**
 * A droplet splitting off a larger shape: it grows out of the point `from`
 * pixels away (toward its parent) on the bouncy spring; on the way out it
 * shrinks back in. Fades under Reduce Motion.
 */
export function splitOff(_node: Element, { from = 20 }: SplitParams = {}, options?: Direction) {
  return directed(options, (out) => {
    if (prefersReducedMotion()) return fadeOnly();
    const css = (t: number, u: number) =>
      `opacity: ${Math.min(1, t * 1.6).toFixed(3)}; translate: ${(u * from).toFixed(2)}px 0; scale: ${(0.4 + 0.6 * t).toFixed(3)};`;
    if (out) return { duration: EXIT_MS + 60, easing: cubicOut, css };
    return { duration: springs.bouncy.duration, easing: springs.bouncy.at, css };
  });
}

/** Delay for the `index`th item of a list entrance: one `--stagger` step each, six steps at most. */
export function staggerMs(index: number, step = 30): number {
  return Math.min(Math.max(index, 0), 6) * step;
}
