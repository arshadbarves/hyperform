import { Box3, Group, Vector3 } from "three";
import type { PetProp } from "../domain/pet/emotions";
import { EASE, phase, sampleTrack, type Stop } from "../domain/pet/keyframes";
import type { PetLook } from "../domain/pet/looks";
import { buildProp, type PropModel } from "./props";
import type { FrameRate } from "./scheduler";
import { disposeTree, toScene } from "./shapes";

/**
 * The prop the pet holds now: swapped when the mood changes, popping in
 * on the bouncy spring from its base (pet-prop-in), then playing its own
 * loop from pet-props.css: the magnifier hunts about, the broom sweeps
 * from the top of its handle, the sign and the paws bob, the laptop's
 * logo glows. Under Reduce Motion it simply appears and holds still.
 */

const POP_MS = 694;
const deg = (d: number) => (d * Math.PI) / 180;
const BOB: Stop[] = [
  [0, 0],
  [0.5, -1.2],
  [1, 0],
];

interface Loop {
  ms: number;
  x?: Stop[];
  y?: Stop[];
  /** Clockwise on screen, about `pivot` (box units). */
  rot?: Stop[];
  pivot?: readonly [number, number];
  glow?: Stop[];
}

const LOOPS: Partial<Record<PetProp, Loop>> = {
  magnifier: {
    ms: 1800,
    x: [[0, 0], [0.3, -1.2], [0.65, 0.6], [1, 0]],
    y: [[0, 0], [0.3, -0.6], [0.65, 0.8], [1, 0]],
  },
  broom: { ms: 900, rot: [[0, deg(-10)], [0.5, deg(12)], [1, deg(-10)]], pivot: [27.2, 11.6] },
  sign: { ms: 1300, y: BOB },
  paws: { ms: 800, y: BOB },
  laptop: { ms: 1400, glow: [[0, 0.2], [0.5, 0.8], [1, 0.2]] },
};

export class PropRig {
  /** Where body props hang (the rig) and where the glasses sit (the face, slid by the gaze). */
  readonly body = new Group();
  readonly face = new Group();
  #prop: PetProp | null = null;
  #look: PetLook | null = null;
  #model: PropModel | null = null;
  #pop = new Group();
  #swing = new Group();
  #base = new Vector3();
  #pivot = new Vector3();
  #since = 0;
  #still = false;

  constructor() {
    this.#pop.add(this.#swing);
  }

  update(prop: PetProp | null, look: PetLook, now: number, still: boolean) {
    this.#still = still;
    if (prop !== this.#prop || (prop === "paws" && look !== this.#look)) this.#swap(prop, look, now);
    const model = this.#model;
    if (!model || !prop) return;
    const t = now - this.#since;
    const k = still ? 1 : EASE.bouncy(phase(t, POP_MS, 1) ?? 1);
    const scale = 0.3 + 0.7 * k;
    this.#pop.scale.setScalar(scale);
    this.#pop.position.copy(this.#base).multiplyScalar(1 - scale);
    this.#pop.position.y -= 1.5 * (1 - k);
    const loop = still ? undefined : LOOPS[prop];
    const u = loop ? (phase(t, loop.ms) ?? 0) : 0;
    const at = (stops: Stop[] | undefined, rest: number) => (loop && stops ? sampleTrack(stops, u, EASE.inOut) : rest);
    this.#swing.position.set(this.#pivot.x + at(loop?.x, 0), this.#pivot.y - at(loop?.y, 0), 0);
    this.#swing.rotation.z = -at(loop?.rot, 0);
    const glow = at(loop?.glow, 0.5);
    for (const material of model.glows) material.opacity = glow;
  }

  #swap(prop: PetProp | null, look: PetLook, now: number) {
    if (this.#model) {
      this.#swing.remove(this.#model.parts);
      disposeTree(this.#model.parts);
    }
    this.#pop.removeFromParent();
    this.#prop = prop;
    this.#look = look;
    this.#since = now;
    this.#model = prop ? buildProp(prop, look) : null;
    if (!this.#model || !prop) return;
    const box = new Box3().setFromObject(this.#model.parts);
    this.#base.set((box.min.x + box.max.x) / 2, box.min.y, (box.min.z + box.max.z) / 2);
    const pivot = LOOPS[prop]?.pivot;
    this.#pivot.copy(pivot ? toScene(pivot[0], pivot[1]) : new Vector3());
    this.#model.parts.position.copy(this.#pivot).negate();
    this.#swing.add(this.#model.parts);
    (this.#model.anchor === "face" ? this.face : this.body).add(this.#pop);
  }

  /** Every frame while it pops in; 30 fps while its loop plays. */
  rate(now: number): FrameRate | null {
    if (!this.#prop || this.#still) return null;
    if (now - this.#since < POP_MS) return "move";
    return LOOPS[this.#prop] ? "loop" : null;
  }

  dispose() {
    if (this.#model) disposeTree(this.#model.parts);
    this.#model = null;
    this.#prop = null;
  }
}
