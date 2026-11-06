import { isPetAccessory, unlockedAccessories, unlocksBetween, type PetAccessory, type PetTrick } from "./looks";

/**
 * The pet's growth, saved in `<data dir>/pet.json`. XP only ever goes up:
 * failures and cancels add nothing, and nothing decays.
 */
export interface PetGrowth {
  version: 1;
  xp: number;
  /** Completed turns, turns whose checks passed, and helpers' changes applied. */
  turns: number;
  verified: number;
  applied: number;
  /** The last local day (YYYY-MM-DD) with a completed turn, and how many days in a row led to it. */
  lastDay: string | null;
  streak: number;
  wearing: PetAccessory | null;
  /** The most recently counted events, so a replayed one never counts twice. */
  counted: string[];
}

export type GrowthEvent =
  | { kind: "turn"; turnId: string; completed: boolean; verified: boolean; day: string }
  | { kind: "applied"; agentId: string; day: string };

export interface GrowthResult {
  state: PetGrowth;
  gained: number;
  levelFrom: number;
  levelTo: number;
  unlocked: { accessories: PetAccessory[]; tricks: PetTrick[] };
}

export const XP = { turn: 10, verified: 15, applied: 20, firstOfDay: 5 } as const;
export const COUNTED_MAX = 200;
export const MAX_LEVEL = 99;

export function emptyGrowth(): PetGrowth {
  return { version: 1, xp: 0, turns: 0, verified: 0, applied: 0, lastDay: null, streak: 0, wearing: null, counted: [] };
}

/** XP needed to reach `level`: 0, 50, 150, 300, 500… (each level asks 50 more than the last). */
export function xpForLevel(level: number): number {
  const l = Math.max(1, Math.min(MAX_LEVEL, Math.floor(level)));
  return 25 * l * (l - 1);
}

export function levelForXp(xp: number): number {
  let level = 1;
  while (level < MAX_LEVEL && xpForLevel(level + 1) <= xp) level += 1;
  return level;
}

/** Where the pet stands inside its level, for the ring on the pet card. */
export function levelProgress(xp: number): { level: number; into: number; span: number; fraction: number } {
  const level = levelForXp(xp);
  const floor = xpForLevel(level);
  const span = level >= MAX_LEVEL ? 1 : xpForLevel(level + 1) - floor;
  const into = Math.max(0, xp - floor);
  return { level, into, span, fraction: level >= MAX_LEVEL ? 1 : Math.min(1, into / span) };
}

/** A local calendar day as YYYY-MM-DD. */
export function localDay(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function dayBefore(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  const prev = new Date(Date.UTC(y ?? 1970, (m ?? 1) - 1, (d ?? 1) - 1));
  return prev.toISOString().slice(0, 10);
}

function count(num: unknown): number {
  return typeof num === "number" && Number.isFinite(num) && num > 0 ? Math.floor(num) : 0;
}

/** Reads what was saved, keeping whatever is valid and defaulting the rest. */
export function parseGrowth(raw: unknown): PetGrowth {
  if (!raw || typeof raw !== "object") return emptyGrowth();
  const o = raw as Record<string, unknown>;
  const day = typeof o.lastDay === "string" && /^\d{4}-\d{2}-\d{2}$/.test(o.lastDay) ? o.lastDay : null;
  const counted = Array.isArray(o.counted) ? o.counted.filter((id): id is string => typeof id === "string") : [];
  return {
    version: 1,
    xp: count(o.xp),
    turns: count(o.turns),
    verified: count(o.verified),
    applied: count(o.applied),
    lastDay: day,
    streak: day ? Math.max(1, count(o.streak)) : 0,
    wearing: isPetAccessory(o.wearing) ? o.wearing : null,
    counted: counted.slice(-COUNTED_MAX),
  };
}

function touchDay(state: PetGrowth, day: string): { state: PetGrowth; bonus: number } {
  if (state.lastDay === day) return { state, bonus: 0 };
  const streak = state.lastDay === dayBefore(day) ? state.streak + 1 : 1;
  return { state: { ...state, lastDay: day, streak }, bonus: XP.firstOfDay };
}

function eventKey(event: GrowthEvent): string {
  return event.kind === "turn" ? `turn:${event.turnId}` : `apply:${event.agentId}`;
}

/** Applies one event; the same event twice counts once. */
export function applyGrowth(state: PetGrowth, event: GrowthEvent): GrowthResult {
  const levelFrom = levelForXp(state.xp);
  const unchanged = { state, gained: 0, levelFrom, levelTo: levelFrom, unlocked: { accessories: [], tricks: [] } };
  const key = eventKey(event);
  if (state.counted.includes(key)) return unchanged;
  if (event.kind === "turn" && !event.completed) return unchanged;

  let gained = event.kind === "turn" ? XP.turn + (event.verified ? XP.verified : 0) : XP.applied;
  const touched = touchDay(state, event.day);
  gained += touched.bonus;
  const next: PetGrowth = {
    ...touched.state,
    xp: state.xp + gained,
    turns: state.turns + (event.kind === "turn" ? 1 : 0),
    verified: state.verified + (event.kind === "turn" && event.verified ? 1 : 0),
    applied: state.applied + (event.kind === "applied" ? 1 : 0),
    counted: [...state.counted, key].slice(-COUNTED_MAX),
  };
  const levelTo = levelForXp(next.xp);
  return { state: next, gained, levelFrom, levelTo, unlocked: unlocksBetween(levelFrom, levelTo) };
}

/** Puts on an unlocked accessory (or takes it off with null); a locked one changes nothing. */
export function wear(state: PetGrowth, accessory: PetAccessory | null): PetGrowth {
  if (accessory === null) return { ...state, wearing: null };
  return unlockedAccessories(levelForXp(state.xp)).includes(accessory) ? { ...state, wearing: accessory } : state;
}
