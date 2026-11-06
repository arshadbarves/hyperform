import type { BodyMotion } from "./emotions";
import type { PetStage } from "./looks";
import type { Point } from "./physics";
import { SEED_SCALE, bodyPose, type RigPose } from "./rig3d";
import type { PetRoutine } from "./routines";

/**
 * Where the island's portrait of the roaming pet looks: its head turns
 * toward the pet's spot on screen and its eyes follow the pet, so the
 * island keeps an eye on it. Positions are viewport pixels.
 */

/** How a pet view frames the pet: all of it with room around, or a close-up of its face. */
export type PetFraming = "full" | "portrait";

/**
 * The close-up of a pet at rest in its 32-unit space: `span` units across,
 * centred on (cx, cy), so its crown fills the round window's top and both
 * eyes stay inside it at the widest turn. The 3D camera and the flat
 * pet's crop both frame this, moved with the body by `portraitFrame`.
 */
export const PORTRAIT_FRAME = { cx: 16, cy: 20, span: 22 } as const;

/**
 * The head turns at most this far to either side (radians), and tips at
 * most PORTRAIT_PITCH up or down, together within the ellipse they span:
 * further and the far eye leaves the round window.
 */
export const PORTRAIT_YAW = (20 * Math.PI) / 180;
export const PORTRAIT_PITCH = (12 * Math.PI) / 180;
/** The eyes reach this share of their usual way toward what they look at, the head having turned already. */
export const PORTRAIT_GAZE = 0.5;
/** The head turns as if the pet were this many px in front of the screen, so a pet close by turns it less. */
const DEPTH = 320;
/** The feet, which the body moves and scales about (rig3d's pivot). */
const FEET = { x: 16, y: 28 } as const;

export type PortraitFrame = { cx: number; cy: number; span: number };

/**
 * The close-up for a body held in `pose` and shrunk to `scale` (the seed):
 * PORTRAIT_FRAME carried along with the body about the feet, so a slumped,
 * sunk, sleeping or leaning pet keeps its face filling the window. Pass
 * the mood's held pose, not its loop, so the loops still move the pet
 * within the window.
 */
export function portraitFrame(pose: Pick<RigPose, "x" | "y" | "rotZ" | "sx" | "sy">, scale = 1): PortraitFrame {
  const dx = (PORTRAIT_FRAME.cx - FEET.x) * pose.sx;
  const dy = (PORTRAIT_FRAME.cy - FEET.y) * pose.sy;
  const [cos, sin] = [Math.cos(pose.rotZ), Math.sin(pose.rotZ)];
  return {
    cx: FEET.x + scale * (dx * cos - dy * sin + pose.x),
    cy: FEET.y + scale * (dx * sin + dy * cos + pose.y),
    span: PORTRAIT_FRAME.span * scale,
  };
}

/** The close-up for a pet of `stage` holding `body` and `routine`'s still pose (the 3D rig's, or the flat pet's CSS). */
export function portraitFor(body: BodyMotion, routine: PetRoutine | null, stage: PetStage): PortraitFrame {
  return portraitFrame(bodyPose(body, routine, 0), stage === "seed" ? SEED_SCALE : 1);
}

export interface PortraitLook {
  /** Radians; > 0 turns the face toward screen right (rig3d's rotY). */
  yaw: number;
  /** Radians; > 0 tips the face down, the top toward the viewer (rig3d's rotX). */
  pitch: number;
  /** Where the eyes look, or null to look straight ahead. */
  lookAt: Point | null;
}

const finite = (v: number) => (Number.isFinite(v) ? v : 0);

/** The portrait in the island slot centred at `slotCenter`, looking toward the roaming pet at `petAt` (null: face forward). */
export function portraitLook(slotCenter: Point, petAt: Point | null): PortraitLook {
  if (!petAt) return { yaw: 0, pitch: 0, lookAt: null };
  const yaw = finite(Math.atan2(petAt.x - slotCenter.x, DEPTH));
  const pitch = finite(Math.atan2(petAt.y - slotCenter.y, DEPTH));
  const reach = Math.max(1, Math.hypot(yaw / PORTRAIT_YAW, pitch / PORTRAIT_PITCH));
  return { yaw: yaw / reach, pitch: pitch / reach, lookAt: { x: petAt.x, y: petAt.y } };
}
