import { LatheGeometry, Mesh, MeshBasicMaterial, PlaneGeometry, Sprite, SpriteMaterial, Vector2, Vector3, type BufferGeometry } from "three";
import type { LiveTone } from "../domain/liveStatus";
import type { PetLook, PetStage } from "../domain/pet/looks";
import { dataTexture, pearl, radialRamp, softEdge } from "./materials";
import { bodyColor, mixColors, petColor, toneColor } from "./palette";
import { PIVOT, ellipsoid, toScene } from "./shapes";

/**
 * The pet's body: an egg turned on a lathe from the right half of the SVG
 * outline (pet BODY in PetFlat), a pearl skin, two feet, the tone-tinted
 * aura behind it and a soft contact shadow under it. Growth tweaks it as
 * the stylesheet does: a bloom glows from inside, a star has a brighter
 * aura and rim. Also the surface math the face and props sit on.
 */

type Bezier = readonly (readonly [number, number])[];

/** The outline's right half, top to its widest (y 19), then down to the bottom. */
const UPPER: Bezier = [[16, 8.6], [22.4, 8.6], [26.6, 13.2], [26.6, 19]];
const LOWER: Bezier = [[26.6, 19], [26.6, 24.2], [22.2, 27.2], [16, 27.2]];
const STEPS = 16;

/** The body is a little shallower than it is wide: a pebble, not a ball. */
export const BODY_DEPTH = 0.9;
/** The height (box y) the body is widest at, where the face turns about. */
export const BODY_MIDDLE = 19;

function bezierAt([p0, p1, p2, p3]: Bezier, t: number): [number, number] {
  const s = 1 - t;
  const mix = (i: 0 | 1) => s * s * s * p0[i] + 3 * s * s * t * p1[i] + 3 * s * t * t * p2[i] + t * t * t * p3[i];
  return [mix(0), mix(1)];
}

/** The outline from the bottom up to the crown, as [radius, box y]; y falls as it climbs. */
export const PROFILE: readonly (readonly [number, number])[] = (() => {
  const out: [number, number][] = [];
  for (let i = STEPS; i >= 0; i--) out.push(bezierAt(LOWER, i / STEPS));
  for (let i = STEPS - 1; i >= 0; i--) out.push(bezierAt(UPPER, i / STEPS));
  return out.map(([x, y]) => [Math.max(0, x - PIVOT.x), y]);
})();

/** The body's radius at box height `y` (0 above its crown and below its base). */
export function radiusAt(y: number): number {
  for (let i = 1; i < PROFILE.length; i++) {
    const [r0, y0] = PROFILE[i - 1];
    const [r1, y1] = PROFILE[i];
    if (y <= y0 && y >= y1) return y0 === y1 ? Math.max(r0, r1) : r0 + ((r1 - r0) * (y - y0)) / (y1 - y0);
  }
  return 0;
}

/** The front of the body at box point (x, y), lifted `out` toward the viewer; at its rim where (x, y) is off the body. */
export function surfacePoint(x: number, y: number, out = 0): Vector3 {
  const at = toScene(x, y);
  const r = radiusAt(y);
  at.z = Math.sqrt(Math.max(0, r * r - at.x * at.x)) * BODY_DEPTH + out;
  return at;
}

/** Which way the front of the body faces at box point (x, y). */
export function surfaceNormal(x: number, y: number): Vector3 {
  const at = surfacePoint(x, y);
  const r = radiusAt(y);
  // The radius's slope as the scene's y rises (the box's y falls).
  const slope = (radiusAt(y - 0.05) - radiusAt(y + 0.05)) / 0.1;
  return new Vector3(at.x, -r * slope, at.z / (BODY_DEPTH * BODY_DEPTH)).normalize();
}

function lathe(grow: number, segments: number, phiStart?: number, phiLength?: number): BufferGeometry {
  const points = PROFILE.map(([r, y]) => new Vector2(r > 0 ? r + grow : 0, PIVOT.y - y));
  const geometry = new LatheGeometry(points, segments, phiStart, phiLength);
  geometry.scale(1, 1, BODY_DEPTH);
  return geometry;
}

/**
 * A thin shell over the front of the body for the face texture, mapped
 * straight on from the front so a point (x, y) of the texture's 32-unit
 * square lands on the body where the SVG face draws it.
 */
export function faceShellGeometry(): BufferGeometry {
  const geometry = lathe(0.06, 40, -1.45, 2.9);
  const position = geometry.getAttribute("position");
  const uv = geometry.getAttribute("uv");
  for (let i = 0; i < position.count; i++) {
    uv.setXY(i, (position.getX(i) + PIVOT.x) / 32, 1 - (PIVOT.y - position.getY(i)) / 32);
  }
  uv.needsUpdate = true;
  return geometry;
}

/** The bloom's inner light: brightest a little below the middle, as the SVG's radial glow. */
function glowMap() {
  const data = new Uint8Array(PROFILE.length * 4);
  PROFILE.forEach(([, y], i) => {
    const v = Math.round(255 * Math.exp(-(((y - 20.1) / 5.4) ** 2)));
    data.set([v, v, v, 255], i * 4);
  });
  return dataTexture(data, 1, PROFILE.length);
}

/** The aura's strength at rest per tone (pet-body.css): quiet dims it, needing you brightens it. */
const AURA: Partial<Record<LiveTone, number>> = { quiet: 0.16, attention: 0.75 };
const AURA_STAR = 0.42;
const AURA_REST = 0.3;
const GLOW: Partial<Record<PetStage, number>> = { bloom: 0.32, star: 0.42 };

export class PetBody {
  readonly skin: Mesh;
  readonly feet: [Mesh, Mesh];
  readonly aura: Sprite;
  readonly shadow: Mesh;
  #skin = pearl(bodyColor("pearl"));
  #foot = pearl(bodyColor("pearl"));
  #aura = new SpriteMaterial({ map: radialRamp(softEdge(0.45)), transparent: true, depthWrite: false });
  #rest = AURA_REST;

  constructor() {
    this.#skin.emissiveMap = glowMap();
    this.skin = new Mesh(lathe(0, 48), this.#skin);
    this.feet = [
      ellipsoid(11.8, 27.2, 2, [2.6, 1.5, 2.4], this.#foot),
      ellipsoid(20.2, 27.2, 2, [2.6, 1.5, 2.4], this.#foot),
    ];
    this.aura = new Sprite(this.#aura);
    this.aura.scale.set(31, 29, 1);
    this.aura.position.copy(toScene(16, 18, -3));
    this.aura.renderOrder = -1;
    const shade = new MeshBasicMaterial({ color: 0x000000, map: radialRamp(softEdge(0.2)), transparent: true, opacity: 0.32, depthWrite: false });
    this.shadow = new Mesh(new PlaneGeometry(1, 1), shade);
    this.shadow.rotation.x = -Math.PI / 2;
    this.shadow.scale.set(19, 14, 1);
    this.shadow.position.copy(toScene(16, 28.3, 1));
  }

  /** Colors for a look, the stage's glow and rim, and the aura's tone. */
  paint(look: PetLook, stage: PetStage, tone: LiveTone) {
    const body = bodyColor(look);
    const light = petColor("light");
    this.#skin.color.copy(mixColors(body, light, 0.9));
    this.#skin.emissive.copy(light);
    this.#skin.emissiveIntensity = GLOW[stage] ?? 0;
    this.#skin.sheen = stage === "star" ? 1 : 0.5;
    this.#foot.color.copy(mixColors(body, petColor("shade"), 0.7));
    this.#aura.color.copy(toneColor(tone, look));
    const rest = AURA[tone] ?? AURA_REST;
    this.#rest = stage === "star" ? Math.max(rest, AURA_STAR) : rest;
  }

  /** The aura's glow now, as a share of its resting strength (rig3d's `aura`). */
  glow(strength: number) {
    this.#aura.opacity = Math.min(1, this.#rest * strength);
  }

  /** Feet planted as the body bobs, each lifted `lift` units. */
  step(bob: number, lift: readonly [number, number]) {
    this.feet.forEach((foot, i) => (foot.position.y = PIVOT.y - 27.2 + bob + lift[i]));
  }
}
