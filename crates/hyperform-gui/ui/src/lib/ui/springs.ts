/**
 * Damped springs turned into CSS easing. A spring is given either physically
 * (`stiffness`, `damping`, `mass`) or the way designers tune it (`response`:
 * the period in seconds, `bounce`: 0 is critically damped, up to 1 bouncier,
 * below 0 overdamped). It starts at rest at 0 and settles at 1.
 *
 * `motion.css` holds the presets as static `--ease-spring-*` / `--dur-spring-*`
 * values printed by `springCssVars()`; `springs.test.ts` fails when they drift.
 */

export type SpringParams =
  | { stiffness: number; damping: number; mass?: number }
  | { response: number; bounce?: number };

export interface SpringCurve {
  /** CSS `linear()` easing sampled over `duration`. */
  easing: string;
  /** Milliseconds until the spring stays within `SETTLE_EPSILON` of its target. */
  duration: number;
  /** Progress for `t` in [0, 1] of `duration`, for Svelte transitions; may pass 1 while it bounces. */
  at: (t: number) => number;
}

/** How close to the target counts as settled: a thousandth of the travel. */
export const SETTLE_EPSILON = 0.001;
const MAX_SECONDS = 10;
const SCAN_STEP = 0.001;
const SAMPLES_PER_SECOND = 60;
const MIN_SAMPLES = 12;

/** The app's springs: snappy for controls, smooth for sheets and panels, bouncy for the pet and droplets, gentle for long travel. */
export const SPRING_PRESETS = {
  snappy: { response: 0.3, bounce: 0.2 },
  smooth: { response: 0.3, bounce: 0 },
  bouncy: { response: 0.42, bounce: 0.35 },
  gentle: { response: 0.5, bounce: 0 },
} as const satisfies Record<string, SpringParams>;

export type SpringPreset = keyof typeof SPRING_PRESETS;

interface Physical {
  stiffness: number;
  damping: number;
  mass: number;
}

/** Stiffness, damping and mass for either way of giving a spring. */
export function springPhysics(params: SpringParams): Physical {
  if ("response" in params) {
    const omega = (2 * Math.PI) / Math.max(params.response, 1e-3);
    const bounce = Math.min(Math.max(params.bounce ?? 0, -0.99), 0.99);
    const ratio = bounce >= 0 ? 1 - bounce : 1 / (1 + bounce);
    return { stiffness: omega * omega, damping: 2 * ratio * omega, mass: 1 };
  }
  return { stiffness: params.stiffness, damping: params.damping, mass: params.mass ?? 1 };
}

/** Position over time (seconds) of a spring released at 0 toward 1. */
export function springPosition(params: SpringParams): (seconds: number) => number {
  const { stiffness, damping, mass } = springPhysics(params);
  const omega = Math.sqrt(stiffness / mass);
  const ratio = damping / (2 * Math.sqrt(stiffness * mass));
  if (Math.abs(ratio - 1) < 1e-6) {
    return (t) => 1 - Math.exp(-omega * t) * (1 + omega * t);
  }
  if (ratio < 1) {
    const damped = omega * Math.sqrt(1 - ratio * ratio);
    const decay = ratio * omega;
    return (t) => 1 - Math.exp(-decay * t) * (Math.cos(damped * t) + (decay / damped) * Math.sin(damped * t));
  }
  const root = Math.sqrt(ratio * ratio - 1);
  const r1 = -omega * (ratio - root);
  const r2 = -omega * (ratio + root);
  return (t) => 1 - (r2 * Math.exp(r1 * t) - r1 * Math.exp(r2 * t)) / (r2 - r1);
}

/** Seconds until the spring stays within `epsilon` of 1 (capped at ten seconds). */
export function settleSeconds(params: SpringParams, epsilon = SETTLE_EPSILON): number {
  const x = springPosition(params);
  let last = 0;
  const steps = Math.round(MAX_SECONDS / SCAN_STEP);
  for (let i = 1; i <= steps; i++) {
    const t = i * SCAN_STEP;
    if (Math.abs(1 - x(t)) > epsilon) last = t;
  }
  return Math.min(last + SCAN_STEP, MAX_SECONDS);
}

function formatPoint(value: number): string {
  const rounded = Math.round(value * 1000) / 1000;
  return Object.is(rounded, -0) ? "0" : String(rounded);
}

/** The spring as CSS `linear()` over `seconds`, about one point per 60 Hz frame. */
export function springEasing(params: SpringParams, seconds = settleSeconds(params)): string {
  const x = springPosition(params);
  const samples = Math.max(MIN_SAMPLES, Math.ceil(seconds * SAMPLES_PER_SECOND));
  const points: string[] = ["0"];
  for (let i = 1; i < samples; i++) points.push(formatPoint(x((i / samples) * seconds)));
  points.push("1");
  return `linear(${points.join(", ")})`;
}

/** Easing, duration and a sampler for one spring. */
export function spring(params: SpringParams): SpringCurve {
  const seconds = settleSeconds(params);
  const x = springPosition(params);
  return {
    easing: springEasing(params, seconds),
    duration: Math.ceil(seconds * 1000),
    at: (t) => (t <= 0 ? 0 : t >= 1 ? 1 : x(t * seconds)),
  };
}

/** Every preset, computed once. */
export const springs: Record<SpringPreset, SpringCurve> = {
  snappy: spring(SPRING_PRESETS.snappy),
  smooth: spring(SPRING_PRESETS.smooth),
  bouncy: spring(SPRING_PRESETS.bouncy),
  gentle: spring(SPRING_PRESETS.gentle),
};

/** The `--ease-spring-*` and `--dur-spring-*` declarations `motion.css` holds, one per line. */
export function springCssVars(): string[] {
  return (Object.keys(springs) as SpringPreset[]).flatMap((name) => [
    `--ease-spring-${name}: ${springs[name].easing};`,
    `--dur-spring-${name}: ${springs[name].duration}ms;`,
  ]);
}
