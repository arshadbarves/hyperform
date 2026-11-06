import { LatheGeometry, Mesh, MeshPhysicalMaterial, Vector2 } from "three";
import type { PetFaceParts } from "../domain/pet/emotions";
import {
  IDENTITY,
  POP_MS,
  YAWN_MS,
  blushLayout,
  browLayout,
  drawnEyeMotion,
  mouthScale,
  popIn,
  sweatDrip,
  tearDrip,
  type BrowLayout,
  type Drip,
} from "../domain/pet/faceLayout";
import { isDrawnEye } from "../domain/pet/faceShapes";
import type { PetStage } from "../domain/pet/looks";
import type { Point } from "../domain/pet/physics";
import { faceShellGeometry, surfaceNormal, surfacePoint } from "./body";
import { FaceTexture, type FaceInk, type FacePaint, type Glyph } from "./faceTexture";
import { mixColors, petColor } from "./palette";
import type { FrameRate } from "./scheduler";
import { moving, retarget, still, valueAt, type Tween } from "./tween";

/**
 * The face on the body: a shell over its front wearing the face texture,
 * slid across the curve by the gaze, plus the sweat drop as a real bead of
 * water by the brow (the SVG draws it past the body's edge, where a face
 * texture cannot reach). It times the face's own motion as pet-face.css
 * does: parts pop in when they change, brows and blush ease between
 * shapes, a talking mouth chatters, a yawn gapes once, drops slide down.
 */

/** The brows morph on the snappy spring, the blush on the smooth one. */
const BROW_MS = 405;
const CHEEK_MS = 441;
/** The right drawn eye starts this much after the left. */
const LATE_MS = 60;
const SWEAT_AT = { x: 24.3, y: 13.8, r: 1.2 };
const STILL_GLYPH: Glyph = { scale: 1, opacity: 1, rot: 0 };

type BrowNumbers = Record<"opacity" | "ldx" | "ldy" | "lrot" | "rdx" | "rdy" | "rrot", number>;
type CheekNumbers = FacePaint["cheeks"];

const flatBrows = ({ opacity, left, right }: BrowLayout): BrowNumbers => ({
  opacity,
  ldx: left.dx,
  ldy: left.dy,
  lrot: left.rot,
  rdx: right.dx,
  rdy: right.dy,
  rrot: right.rot,
});
const fullBrows = (b: BrowNumbers): BrowLayout => ({
  opacity: b.opacity,
  left: { ...IDENTITY, dx: b.ldx, dy: b.ldy, rot: b.lrot },
  right: { ...IDENTITY, dx: b.rdx, dy: b.rdy, rot: b.rrot },
});

/** A water drop with its round end centred on the origin and its tip `2r` above. */
function dropGeometry(r: number): LatheGeometry {
  const points: Vector2[] = [];
  for (let i = 0; i <= 10; i++) {
    const a = -Math.PI / 2 + (i / 10) * Math.PI * 0.72;
    points.push(new Vector2(Math.max(0, Math.cos(a) * r), Math.sin(a) * r));
  }
  points.push(new Vector2(0, r * 2));
  return new LatheGeometry(points, 20);
}

export interface FaceState {
  face: PetFaceParts;
  stage: PetStage;
  /** The gaze offset, in box units. */
  gaze: Point;
  /** Reduce Motion: parts appear at once and nothing loops. */
  still: boolean;
}

export class PetFace {
  readonly shell: Mesh;
  readonly sweat: Mesh;
  #texture = new FaceTexture();
  #ink: FaceInk;
  #sweatSkin: MeshPhysicalMaterial;
  #sweatRest = surfacePoint(SWEAT_AT.x, SWEAT_AT.y).addScaledVector(surfaceNormal(SWEAT_AT.x, SWEAT_AT.y), SWEAT_AT.r * 0.8);
  #last: PetFaceParts | null = null;
  #since = { eyes: 0, mouth: 0, sweat: 0, tears: 0 };
  #brows: Tween<BrowNumbers> = still(flatBrows(browLayout("none")));
  #cheeks: Tween<CheekNumbers> = still(blushLayout("none"));
  #still = false;

  constructor() {
    const eye = petColor("eye");
    const blush = petColor("blush");
    this.#ink = { eye, blush, mouth: mixColors(eye, blush, 0.86), gold: petColor("gold"), heart: petColor("heart"), sweat: petColor("sweat") };
    const skin = new MeshPhysicalMaterial({
      map: this.#texture.texture,
      transparent: true,
      depthWrite: false,
      roughness: 0.5,
      clearcoat: 0.5,
      clearcoatRoughness: 0.3,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -2,
    });
    this.shell = new Mesh(faceShellGeometry(), skin);
    this.shell.visible = this.#texture.texture !== null;
    this.#sweatSkin = new MeshPhysicalMaterial({ color: this.#ink.sweat, roughness: 0.05, clearcoat: 1, transparent: true, depthWrite: false });
    this.sweat = new Mesh(dropGeometry(SWEAT_AT.r), this.#sweatSkin);
    this.sweat.visible = false;
  }

  update(state: FaceState, now: number) {
    const { face } = state;
    this.#still = state.still;
    this.#note(face, now, state.stage);
    const t = (since: number) => (state.still ? Infinity : now - since);
    this.#texture.paint(
      {
        cheeks: valueAt(this.#cheeks, now),
        brows: fullBrows(valueAt(this.#brows, now)),
        eyes: isDrawnEye(face.eyes)
          ? { shape: face.eyes, left: this.#glyph(face, t(this.#since.eyes)), right: this.#glyph(face, t(this.#since.eyes) - LATE_MS) }
          : null,
        mouth: this.#mouth(face, t(this.#since.mouth)),
        tears: face.tears ? { left: this.#drip(t(this.#since.tears), "left"), right: this.#drip(t(this.#since.tears), "right") } : null,
      },
      this.#ink,
    );
    this.#texture.texture?.offset.set(-state.gaze.x / 32, state.gaze.y / 32);
    const sweat = face.sweat ? (state.still ? { opacity: 0.9, dy: 0 } : sweatDrip(now - this.#since.sweat)) : null;
    this.sweat.visible = !!sweat && sweat.opacity > 0.01;
    if (sweat) {
      this.sweat.position.copy(this.#sweatRest).setY(this.#sweatRest.y - sweat.dy);
      this.#sweatSkin.opacity = sweat.opacity;
    }
  }

  /** Remembers when each part changed; brows and blush head for their new shape. */
  #note(face: PetFaceParts, now: number, stage: PetStage) {
    const last = this.#last;
    const first = !last;
    if (first || face.eyes !== last.eyes) this.#since.eyes = now;
    if (first || face.mouth !== last.mouth) this.#since.mouth = now;
    if (face.sweat && (first || !last.sweat)) this.#since.sweat = now;
    if (face.tears && (first || !last.tears)) this.#since.tears = now;
    this.#last = face;
    const quick = first || this.#still;
    this.#brows = retarget(this.#brows, flatBrows(browLayout(face.brows)), now, quick ? 0 : BROW_MS);
    this.#cheeks = retarget(this.#cheeks, blushLayout(face.blush, stage), now, quick ? 0 : CHEEK_MS);
  }

  #glyph(face: PetFaceParts, tMs: number): Glyph {
    if (tMs === Infinity) return STILL_GLYPH;
    const pop = popIn("eyes", tMs);
    const motion = drawnEyeMotion(face.eyes, tMs);
    return { scale: pop.scale * motion.scale, opacity: pop.opacity, rot: motion.rot };
  }

  #mouth(face: PetFaceParts, tMs: number): FacePaint["mouth"] {
    if (face.mouth === "none") return null;
    const settled = tMs === Infinity;
    const pop = settled ? { scale: 1, opacity: 1 } : popIn("mouth", tMs);
    // Held still, a talking mouth rests open and a yawn keeps its last frame, as CSS leaves them.
    const scale = settled && face.mouth !== "yawn" ? { ...mouthScale(face.mouth, 0), sx: 1, sy: 1 } : mouthScale(face.mouth, settled ? YAWN_MS : tMs);
    return { shape: face.mouth, pop: pop.scale, opacity: pop.opacity, ...scale };
  }

  #drip(tMs: number, side: "left" | "right"): Drip {
    return tMs === Infinity ? { opacity: 1, dy: 0 } : tearDrip(side, tMs);
  }

  /** "move" while a part pops in or morphs, "loop" while one loops (talking, drawn eyes, drops). */
  rate(now: number): FrameRate | null {
    const face = this.#last;
    if (!face || this.#still) return null;
    const popping =
      (isDrawnEye(face.eyes) && now - this.#since.eyes < POP_MS.eyes + LATE_MS) ||
      (face.mouth !== "none" && now - this.#since.mouth < (face.mouth === "yawn" ? YAWN_MS : POP_MS.mouth));
    if (popping || moving(this.#brows, now) || moving(this.#cheeks, now)) return "move";
    const loops = face.mouth === "talk" || face.eyes === "star" || face.eyes === "heart" || face.eyes === "spiral";
    return loops || face.sweat || face.tears ? "loop" : null;
  }
}
