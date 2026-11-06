import { Group, Mesh, SphereGeometry, Vector3 } from "three";
import type { PetLook } from "../domain/pet/looks";
import { glossy } from "./materials";
import { bodyColor, mixColors, toneColor } from "./palette";
import type { FrameRate } from "./scheduler";
import { toScene } from "./shapes";

/**
 * The running helpers (agents and jobs) as little orbs circling the pet on
 * a tilted ring, passing behind it at the top and in front at the bottom:
 * the SVG's orbiting sprites (`pet-orbit`), at most four, a quarter turn
 * apart, once round every six seconds.
 */

export const MAX_HELPERS = 4;
const ORBIT_MS = 6000;
const RADIUS = 14;
const TILT = 0.42;
const CENTRE = toScene(16, 18);

/** Where orb `i` of the ring is at `tMs`, in the scene. */
export function orbitAt(i: number, tMs: number): Vector3 {
  const a = 2 * Math.PI * (tMs / ORBIT_MS + i / MAX_HELPERS);
  const x = Math.sin(a) * RADIUS;
  const z = Math.cos(a) * RADIUS;
  return new Vector3(x, CENTRE.y - z * Math.sin(TILT), z * Math.cos(TILT));
}

export class HelperOrbs {
  readonly group = new Group();
  #orbs: Mesh[] = [];
  #skin = glossy(bodyColor("pearl"), 0.3);
  #count = 0;

  constructor() {
    const geometry = new SphereGeometry(1.5, 20, 14);
    for (let i = 0; i < MAX_HELPERS; i++) {
      const orb = new Mesh(geometry, this.#skin);
      orb.visible = false;
      this.#orbs.push(orb);
      this.group.add(orb);
    }
  }

  /** The orbs take a touch of blue over the body's color, as the sprites do. */
  paint(look: PetLook) {
    this.#skin.color.copy(mixColors(toneColor("working", look), bodyColor(look), 0.35));
  }

  /** Shows `count` orbs where they are at `now`; held still, they wait a quarter turn apart. */
  update(count: number, now: number, still: boolean) {
    this.#count = Math.min(MAX_HELPERS, Math.max(0, Math.floor(count)));
    this.#orbs.forEach((orb, i) => {
      orb.visible = i < this.#count;
      if (orb.visible) orb.position.copy(orbitAt(i, still ? 0 : now));
    });
  }

  rate(): FrameRate | null {
    return this.#count > 0 ? "loop" : null;
  }
}
