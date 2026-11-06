/**
 * Where the pet can be. A perch is either a slot the pet sits inside (the
 * island's orb, the hero spot on Home, an empty state, the free space in the
 * side panel's tab band) or the top edge of a floating pane it stands on
 * (the composer, the sidebar footer).
 */
export type PerchId = "island" | "hero" | "empty" | "composer" | "sidebar" | "panel";
export type PerchKind = "slot" | "edge";

export interface PerchRect {
  id: PerchId;
  kind: PerchKind;
  left: number;
  top: number;
  width: number;
  height: number;
}

/** The pet's center x, the y its feet rest on, and its drawn size. */
export interface PetSpot {
  x: number;
  y: number;
  size: number;
}

/** Where the pet's feet rest, as a share of its drawn size from the top. */
export const PET_FEET = 28.2 / 32;

/** How big the pet is on each perch, in pixels. */
export const PERCH_SIZE: Record<PerchId, number> = {
  island: 28,
  composer: 46,
  sidebar: 40,
  panel: 26,
  hero: 72,
  empty: 64,
};

/** Perches on the stage, which the expanded side panel covers. */
export const STAGE_PERCHES: ReadonlySet<PerchId> = new Set<PerchId>(["hero", "empty", "composer"]);

/** Keeps the pet's feet this far from an edge's ends. */
const EDGE_INSET = 18;

/** The pet on a perch: centered in a slot, or standing on an edge `along` of the way across (0..1). */
export function spotOn(perch: PerchRect, along: number, size = PERCH_SIZE[perch.id]): PetSpot {
  if (perch.kind === "slot") {
    return { x: perch.left + perch.width / 2, y: perch.top + perch.height / 2 + size / 2, size };
  }
  const min = perch.left + EDGE_INSET + size / 2;
  const max = perch.left + perch.width - EDGE_INSET - size / 2;
  const t = Math.min(1, Math.max(0, along));
  const x = max <= min ? perch.left + perch.width / 2 : min + (max - min) * t;
  return { x, y: perch.top, size };
}

/** The `along` on an edge perch that puts the pet nearest to `x`. */
export function alongFor(perch: PerchRect, x: number, size = PERCH_SIZE[perch.id]): number {
  if (perch.kind === "slot") return 0.5;
  const min = perch.left + EDGE_INSET + size / 2;
  const max = perch.left + perch.width - EDGE_INSET - size / 2;
  if (max <= min) return 0.5;
  return Math.min(1, Math.max(0, (x - min) / (max - min)));
}

/** Where a dropped pet lands: the perch whose resting spot is closest to the drop point. */
export function nearestPerch(perches: readonly PerchRect[], x: number, y: number): PerchRect | null {
  let best: PerchRect | null = null;
  let bestDistance = Infinity;
  for (const perch of perches) {
    const spot = spotOn(perch, alongFor(perch, x));
    const distance = Math.hypot(spot.x - x, spot.y - spot.size / 2 - y);
    if (distance < bestDistance) {
      best = perch;
      bestDistance = distance;
    }
  }
  return best;
}

/** Whether a rect is on screen and big enough to hold the pet (a slot mid-morph may run a little small). */
export function usable(perch: PerchRect, viewport: { width: number; height: number }): boolean {
  if (perch.width <= 0 || perch.height <= 0) return false;
  const visible = perch.left < viewport.width && perch.left + perch.width > 0 && perch.top < viewport.height && perch.top + perch.height > 0;
  if (!visible) return false;
  const size = PERCH_SIZE[perch.id];
  if (perch.kind === "slot") return Math.min(perch.width, perch.height) >= size * 0.75;
  return perch.width >= size + EDGE_INSET * 2;
}
