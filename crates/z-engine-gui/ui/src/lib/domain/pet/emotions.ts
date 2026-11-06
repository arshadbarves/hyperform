import type { Particles, PetMood, PetPose } from "./pose";

/**
 * How each mood looks: the face (eyes, brows, mouth, blush, sweat, tears),
 * how the body moves, what the pet holds and what floats above it. PetFace,
 * PetProps and PetBubble draw these; pose.ts decides the mood.
 */

/** Eyes drawn as ellipses that morph between shapes, or a drawn shape. */
export type EyeShape =
  | "open"
  | "wide"
  | "narrow"
  | "half"
  | "closed"
  | "sad"
  | "puppy"
  | "uneven"
  | "happy"
  | "star"
  | "heart"
  | "spiral";
export type BrowShape = "none" | "raised" | "relaxed" | "worried" | "sad" | "firm" | "uneven";
export type MouthShape =
  | "none"
  | "smile"
  | "grin"
  | "laugh"
  | "o"
  | "flat"
  | "frown"
  | "wobble"
  | "yawn"
  | "tongue"
  | "cat"
  | "smug"
  | "talk";
export type Blush = "none" | "soft" | "strong";

export interface PetFaceParts {
  eyes: EyeShape;
  brows: BrowShape;
  mouth: MouthShape;
  blush: Blush;
  sweat: boolean;
  tears: boolean;
}

/** A looping (or one-shot) body motion; each is a keyframe in pet-keyframes.css. */
export type BodyMotion =
  | "breathe"
  | "bob"
  | "sway"
  | "nod"
  | "lean"
  | "type"
  | "tilt"
  | "sweep"
  | "bounce"
  | "puff"
  | "sigh"
  | "slump"
  | "tremble"
  | "squish"
  | "wiggle"
  | "wave"
  | "sink"
  | "yawn"
  | "doze"
  | "sleep";

export type PetProp = "glasses" | "magnifier" | "laptop" | "clipboard" | "broom" | "sign" | "paws";
/** What floats above the pet: a bubble with a glyph (? ! … idea) or loose zzz, hearts and notes. */
export type BubbleKind = "question" | "exclaim" | "dots" | "idea" | "zzz" | "hearts" | "notes";
/** Bursts around it: sparkles, confetti, or stars circling a dizzy head. */
export type PetFx = "none" | "sparkles" | "confetti" | "stars";

export interface Emotion {
  face: PetFaceParts;
  body: BodyMotion;
  prop: PetProp | null;
  bubble: BubbleKind | null;
  fx: PetFx;
}

type Row = [EyeShape, BrowShape, MouthShape, Blush, BodyMotion, PetProp | null, BubbleKind | null, PetFx?, ("sweat" | "tears")?];

const ROWS: Record<PetMood, Row> = {
  // the agent's work
  thinking: ["open", "uneven", "flat", "none", "sway", null, "dots"],
  reading: ["narrow", "relaxed", "flat", "none", "nod", "glasses", null],
  searching: ["wide", "raised", "o", "none", "lean", "magnifier", null],
  coding: ["narrow", "firm", "tongue", "none", "type", "laptop", null],
  talking: ["open", "relaxed", "talk", "soft", "bob", null, null],
  busy: ["open", "firm", "flat", "none", "bob", null, null],
  planning: ["open", "raised", "smile", "none", "tilt", "clipboard", "idea"],
  tidying: ["happy", "relaxed", "cat", "soft", "sweep", "broom", null],
  determined: ["narrow", "firm", "flat", "none", "bob", null, null, "none", "sweat"],
  // it needs you
  asking: ["wide", "raised", "o", "none", "tilt", null, "question"],
  pleading: ["puppy", "worried", "wobble", "soft", "bounce", "paws", null],
  presenting: ["happy", "raised", "grin", "soft", "puff", "sign", null],
  // how it turned out
  proud: ["happy", "relaxed", "smug", "soft", "puff", null, null, "sparkles"],
  relieved: ["closed", "relaxed", "smile", "soft", "sigh", null, null, "none", "sweat"],
  excited: ["star", "raised", "laugh", "strong", "bounce", null, null, "sparkles"],
  happy: ["happy", "none", "smile", "soft", "squish", null, null],
  content: ["happy", "relaxed", "cat", "soft", "breathe", null, "notes"],
  sad: ["sad", "sad", "frown", "none", "slump", null, null, "none", "tears"],
  worried: ["open", "worried", "wobble", "none", "tremble", null, null, "none", "sweat"],
  confused: ["uneven", "uneven", "wobble", "none", "tilt", null, "question"],
  // you, and idle time
  surprised: ["wide", "raised", "o", "none", "tremble", null, "exclaim"],
  dizzy: ["spiral", "none", "wobble", "none", "wiggle", null, null, "stars"],
  love: ["heart", "none", "grin", "strong", "squish", null, "hearts"],
  giggle: ["happy", "relaxed", "laugh", "strong", "wiggle", null, null],
  greeting: ["happy", "raised", "grin", "soft", "wave", null, null, "sparkles"],
  listening: ["open", "relaxed", "smile", "none", "nod", null, null],
  watching: ["wide", "raised", "smile", "none", "lean", null, null],
  curious: ["open", "raised", "o", "none", "tilt", null, null],
  peeking: ["open", "raised", "none", "none", "sink", null, null],
  bored: ["half", "none", "flat", "none", "sigh", null, "dots"],
  yawning: ["closed", "relaxed", "yawn", "none", "yawn", null, null],
  sleepy: ["half", "relaxed", "flat", "none", "doze", null, null],
  asleep: ["closed", "none", "o", "soft", "sleep", null, "zzz"],
  idle: ["open", "none", "none", "none", "breathe", null, null],
};

export const PET_MOODS = Object.keys(ROWS) as PetMood[];

export function emotion(mood: PetMood): Emotion {
  const [eyes, brows, mouth, blush, body, prop, bubble, fx = "none", drops] = ROWS[mood];
  return { face: { eyes, brows, mouth, blush, sweat: drops === "sweat", tears: drops === "tears" }, body, prop, bubble, fx };
}

/** A pose's `particles` ask for extras on top of the mood's own (older callers still pass them). */
const EXTRA: Record<Particles, Partial<Pick<Emotion, "bubble" | "fx">>> = {
  none: {},
  thought: { bubble: "dots" },
  sleep: { bubble: "zzz" },
  hearts: { bubble: "hearts" },
  sparkles: { fx: "sparkles" },
  confetti: { fx: "confetti" },
};

/** Everything the pet shows for a pose: its mood's emotion, with any extra particles on top. */
export function expression(pose: Pick<PetPose, "mood" | "particles">): Emotion {
  return { ...emotion(pose.mood), ...EXTRA[pose.particles] };
}

/** Eyes that can blink (and follow a point): the open ellipse shapes. */
export function canBlink(eyes: EyeShape): boolean {
  return eyes === "open" || eyes === "wide" || eyes === "narrow" || eyes === "puppy" || eyes === "uneven";
}
