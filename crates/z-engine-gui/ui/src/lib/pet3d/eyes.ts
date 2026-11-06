import { Group, Mesh, Quaternion, SphereGeometry, Vector3 } from "three";
import type { EyeShape } from "../domain/pet/emotions";
import { BLINK, BLINK_MS, EYE_MORPH_MS, IDENTITY, eyeLayout, showsGlint, type PartTransform } from "../domain/pet/faceLayout";
import { EYES, GLINTS, type Side } from "../domain/pet/faceShapes";
import type { PetStage } from "../domain/pet/looks";
import type { Point } from "../domain/pet/physics";
import { surfaceNormal, surfacePoint } from "./body";
import { glossy, unlit } from "./materials";
import { petColor } from "./palette";
import type { FrameRate } from "./scheduler";
import { moving, restart, retarget, still, valueAt, type Tween } from "./tween";

/**
 * The ellipse eyes as glossy beads set into the skin (open, wide, narrow,
 * half, sad, puppy, uneven). They morph between shapes by scale, turn and
 * offset (faceLayout's eyeLayout), blink by squeezing flat, open from a
 * squint when they replace drawn eyes, slide over the curved body with the
 * gaze, and puppy eyes catch a glint. Drawn eyes are the face texture's.
 */

const SIDES: readonly Side[] = ["left", "right"];
/** The beads are this deep for their width, and sunk this far into the skin. */
const DEPTH = 0.6;
const SUNK = 0.25;
/** Eyes that replace drawn ones open from a squint (pet-face.css `pet-eye-open`). */
const OPEN_MS = 220;
const SQUINT: PartTransform = { ...IDENTITY, sy: 0.15 };
const FRONT = new Vector3(0, 0, 1);

export interface EyesState {
  eyes: EyeShape;
  stage: PetStage;
  blinking: boolean;
  /** The gaze offset, in box units. */
  gaze: Point;
  /** Reduce Motion: every change is instant. */
  still: boolean;
}

export class PetEyes {
  readonly group = new Group();
  #beads: Record<Side, Mesh>;
  #glints: Record<Side, Mesh>;
  #tweens: Record<Side, Tween<PartTransform>> = { left: still(IDENTITY), right: still(IDENTITY) };
  #shown = false;
  #spin = new Quaternion();

  constructor() {
    const bead = new SphereGeometry(1, 28, 18);
    const ink = glossy(petColor("eye"), 0.14);
    const light = unlit(petColor("light"), 0.92);
    const make = (material: typeof ink | typeof light) => new Mesh(bead, material);
    this.#beads = { left: make(ink), right: make(ink) };
    this.#glints = { left: make(light), right: make(light) };
    for (const side of SIDES) {
      this.#glints[side].scale.set(GLINTS[side].r, GLINTS[side].r, GLINTS[side].r * 0.5);
      this.group.add(this.#beads[side], this.#glints[side]);
    }
  }

  update(state: EyesState, now: number) {
    const layout = eyeLayout(state.eyes, state.stage);
    const shown = !layout.drawn;
    const opening = shown && !this.#shown && !state.still;
    this.#shown = shown;
    const glint = shown && showsGlint(state.eyes, state.blinking);
    for (const side of SIDES) {
      const target = state.blinking && shown ? BLINK : layout[side];
      const ms = state.still ? 0 : state.blinking ? BLINK_MS : EYE_MORPH_MS;
      this.#tweens[side] = opening ? restart(SQUINT, target, now, OPEN_MS) : retarget(this.#tweens[side], target, now, ms);
      this.#beads[side].visible = shown;
      this.#glints[side].visible = glint;
      if (shown) this.#place(side, valueAt(this.#tweens[side], now), state.gaze);
    }
  }

  #place(side: Side, pose: PartTransform, gaze: Point) {
    const eye = EYES[side];
    const x = eye.cx + pose.dx + gaze.x;
    const y = eye.cy + pose.dy + gaze.y;
    const normal = surfaceNormal(x, y);
    const bead = this.#beads[side];
    bead.position.copy(surfacePoint(x, y)).addScaledVector(normal, -SUNK);
    bead.quaternion.setFromUnitVectors(FRONT, normal).multiply(this.#spin.setFromAxisAngle(FRONT, -pose.rot));
    bead.scale.set(eye.rx * pose.sx, eye.ry * pose.sy, eye.rx * DEPTH);
    const g = GLINTS[side];
    const at = { x: g.cx + gaze.x, y: g.cy + gaze.y };
    this.#glints[side].position.copy(surfacePoint(at.x, at.y)).addScaledVector(surfaceNormal(at.x, at.y), eye.rx * DEPTH - SUNK + 0.1);
  }

  /** The eyes are morphing, blinking or opening: every frame until they settle. */
  rate(now: number): FrameRate | null {
    return SIDES.some((side) => moving(this.#tweens[side], now)) ? "move" : null;
  }
}
