import { DirectionalLight, Group, HemisphereLight, PerspectiveCamera, Scene, type Texture, type Vector3 } from "three";
import type { LiveTone } from "../domain/liveStatus";
import type { BodyMotion, PetFaceParts, PetProp } from "../domain/pet/emotions";
import { scanOffset } from "../domain/pet/faceLayout";
import type { PetAccessory, PetLook, PetStage } from "../domain/pet/looks";
import type { Point } from "../domain/pet/physics";
import type { PetMood } from "../domain/pet/pose";
import { PORTRAIT_FRAME, PORTRAIT_GAZE, portraitFor, type PetFraming, type PortraitFrame } from "../domain/pet/portrait";
import { SEED_SCALE, animates, bodyPose, faceYaw, threeAxes } from "../domain/pet/rig3d";
import type { PetRoutine } from "../domain/pet/routines";
import { buildAccessory } from "./accessories";
import { PetBody } from "./body";
import { PetEyes } from "./eyes";
import { PetFace } from "./face";
import { HelperOrbs } from "./helpers";
import { PropRig } from "./propRig";
import type { FrameRate } from "./scheduler";
import { disposeTree, toScene } from "./shapes";

/**
 * One pet in 3D: its body, face, eyes, prop, accessory and helper orbs
 * under a rig that plays the SVG pet's moves (rig3d.ts), lit from the top
 * left like its painted shine, seen through a narrow perspective camera.
 * `update` poses it for a moment; `rate` says whether it needs frames.
 *
 * Nesting, from the feet up: turn (facing, the portrait's head turn, the
 * seed's smaller size) > tip (the portrait's nod, about the body's middle)
 * > stride (the walk's bob) > rig (the mood and routine) > the body.
 */

/** The camera's vertical field of view, in degrees: narrow, so the pet is not distorted. */
export const FOV = 22;
/** The full view spans this many units: the 32-unit box with a quarter spare each side, so a canvas 1.5× the box fits it. */
export const FULL_SPAN = 48;

export interface PetSceneState {
  face: PetFaceParts;
  body: BodyMotion;
  prop: PetProp | null;
  mood: PetMood;
  tone: LiveTone;
  look: PetLook;
  stage: PetStage;
  wearing: PetAccessory | null;
  routine: PetRoutine | null;
  helpers: number;
  /** The eyes' gaze offset (usePetEyes), box units. */
  gaze: Point;
  /** The gaze sweeps side to side (faster while searching). */
  scan: boolean;
  searching: boolean;
  blinking: boolean;
  /** The motion engine's facing, -1 (left) .. 1 (right); 0 faces the viewer. */
  turn: number;
  /** Feet lift [left, right] and body bob, box units. */
  lift: readonly [number, number];
  bob: number;
  /** The portrait's head turn and nod, radians (portrait.ts). */
  head: { yaw: number; pitch: number } | null;
  /** Reduce Motion: a still pose, drawn once per change. */
  still: boolean;
}

/** Points `camera` at the framed part of the box: all of it with room around, or the face's close-up (`portrait`, as the body is held). */
export function frameCamera(camera: PerspectiveCamera, framing: PetFraming, portrait: PortraitFrame = PORTRAIT_FRAME) {
  const { cx, cy, span } = framing === "portrait" ? portrait : { cx: 16, cy: 16, span: FULL_SPAN };
  const target = toScene(cx, cy);
  const distance = span / 2 / Math.tan((FOV * Math.PI) / 360);
  camera.fov = FOV;
  camera.aspect = 1;
  camera.near = Math.max(1, distance - 40);
  camera.far = distance + 40;
  camera.position.set(target.x, target.y, distance);
  camera.lookAt(target);
  camera.updateProjectionMatrix();
}

/** The quicker of the frame rates asked for. */
export function faster(...rates: (FrameRate | null)[]): FrameRate | null {
  return rates.includes("move") ? "move" : rates.includes("loop") ? "loop" : null;
}

const TIP_AT = toScene(16, 18);

export class PetScene {
  readonly scene = new Scene();
  readonly camera = new PerspectiveCamera();
  readonly turn = new Group();
  readonly tip = new Group();
  readonly stride = new Group();
  readonly rig = new Group();
  readonly body = new PetBody();
  #eyes = new PetEyes();
  #face = new PetFace();
  #props = new PropRig();
  #helpers = new HelperOrbs();
  /** Slides what sits on the face (the glasses) with the gaze. */
  #gazed = new Group();
  #wear: { kind: PetAccessory; model: Group } | null = null;
  #last: PetSceneState | null = null;
  #since = { body: 0, routine: 0, scan: 0 };
  /** Undoes the tip's offset, so the tip nods about its own position. */
  #inner = new Group();
  #framing: PetFraming = "full";
  #frame = "";

  constructor(framing: PetFraming = "full") {
    this.setFraming(framing);
    const key = new DirectionalLight(0xffffff, 2.3);
    key.position.set(-14, 26, 22);
    const rim = new DirectionalLight(0xffffff, 1.5);
    rim.position.set(12, 16, -20);
    this.scene.add(new HemisphereLight(0xffffff, 0x8a8f9f, 0.55), key, rim);
    this.scene.environmentIntensity = 0.55;

    this.tip.add(this.#inner);
    this.#inner.add(this.stride);
    this.stride.add(this.rig);
    this.#gazed.add(this.#props.face);
    this.rig.add(this.body.skin, ...this.body.feet, this.body.aura, this.#face.shell, this.#face.sweat, this.#eyes.group, this.#gazed, this.#props.body);
    this.turn.add(this.body.shadow, this.tip);
    this.scene.add(this.turn, this.#helpers.group);
  }

  /** Frames all of the pet, or its face's close-up, which follows the body as it is posed. */
  setFraming(framing: PetFraming) {
    this.#framing = framing;
    this.#frame = "";
    frameCamera(this.camera, framing);
    this.#tipAt(TIP_AT);
    if (framing === "portrait" && this.#last) this.#follow(this.#last);
  }

  /** The reflection map (renderer's `environment()`), set again after the context comes back. */
  setEnvironment(texture: Texture | null) {
    this.scene.environment = texture;
  }

  /** Poses the pet as `state` says, `now` ms on the frame clock. */
  update(state: PetSceneState, now: number) {
    const last = this.#last;
    if (!last || last.look !== state.look || last.stage !== state.stage || last.tone !== state.tone) {
      this.body.paint(state.look, state.stage, state.tone);
      this.#helpers.paint(state.look);
    }
    if (!last || last.body !== state.body) this.#since.body = now;
    if (!last || last.routine !== state.routine) this.#since.routine = now;
    if (state.scan && !last?.scan) this.#since.scan = now;
    this.#wearing(state.wearing);
    this.#last = state;

    const pose = state.still
      ? bodyPose(state.body, state.routine, 0)
      : bodyPose(state.body, state.routine, now - this.#since.body, { routineMs: now - this.#since.routine, mood: state.mood });
    const { position, rotation, scale } = threeAxes(pose);
    this.rig.position.set(...position);
    this.rig.rotation.set(...rotation);
    this.rig.scale.set(...scale);
    this.stride.position.y = -state.bob;
    this.body.step(state.bob, [state.lift[0] + pose.lift[0], state.lift[1] + pose.lift[1]]);
    this.body.glow(pose.aura);
    this.turn.rotation.y = faceYaw(state.turn) + (state.head?.yaw ?? 0);
    this.turn.scale.setScalar(state.stage === "seed" ? SEED_SCALE : 1);
    this.tip.rotation.x = state.head?.pitch ?? 0;
    if (this.#framing === "portrait") this.#follow(state);

    const sweep = state.scan && !state.still ? scanOffset(now - this.#since.scan, state.searching) : { x: 0, y: 0 };
    const reach = this.#framing === "portrait" ? PORTRAIT_GAZE : 1;
    const gaze = { x: (state.gaze.x + sweep.x) * reach, y: (state.gaze.y + sweep.y) * reach };
    this.#eyes.update({ eyes: state.face.eyes, stage: state.stage, blinking: state.blinking, gaze, still: state.still }, now);
    this.#face.update({ face: state.face, stage: state.stage, gaze, still: state.still }, now);
    this.#gazed.position.set(gaze.x, -gaze.y, 0);
    this.#props.update(state.prop, state.look, now, state.still);
    this.#helpers.update(state.helpers, now, state.still);
  }

  #tipAt(at: Vector3) {
    this.tip.position.copy(at);
    this.#inner.position.copy(at).negate();
  }

  /** Keeps the portrait on the face as the mood's held pose moves it, and nods the head about the face's middle. */
  #follow(state: PetSceneState) {
    const frame = portraitFor(state.body, state.routine, state.stage);
    const key = `${frame.cx.toFixed(3)},${frame.cy.toFixed(3)},${frame.span.toFixed(3)}`;
    if (key === this.#frame) return;
    this.#frame = key;
    frameCamera(this.camera, "portrait", frame);
    // The tip sits inside the seed's scale, so it nods about the unscaled middle.
    const middle = portraitFor(state.body, state.routine, "sprout");
    this.#tipAt(toScene(middle.cx, middle.cy));
  }

  #wearing(kind: PetAccessory | null) {
    if (this.#wear?.kind === kind) return;
    if (this.#wear) {
      this.rig.remove(this.#wear.model);
      disposeTree(this.#wear.model);
    }
    this.#wear = kind ? { kind, model: buildAccessory(kind) } : null;
    if (this.#wear) this.rig.add(this.#wear.model);
  }

  /**
   * The frames the pet needs after the last update, at `now`: every frame
   * while a routine, a morph or a pop-in plays; 30 fps while a mood loops,
   * the face loops, a prop loops, the gaze sweeps or helpers orbit; none
   * while it only breathes (a CSS loop) or holds still.
   */
  rate(now: number): FrameRate | null {
    const state = this.#last;
    if (!state || state.still) return null;
    const moving = !!state.routine && animates("breathe", state.routine, now - this.#since.routine);
    const looping = animates(state.body, null, now - this.#since.body, { mood: state.mood }) || state.scan;
    return faster(moving ? "move" : null, this.#eyes.rate(now), this.#face.rate(now), this.#props.rate(now), this.#helpers.rate(), looping ? "loop" : null);
  }

  dispose() {
    this.#props.dispose();
    disposeTree(this.scene);
    this.#wear = null;
  }
}
