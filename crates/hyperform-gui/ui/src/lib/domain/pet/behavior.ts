import type { LiveStatus } from "../liveStatus";
import type { StageView } from "../stage";
import { PERCH_SIZE, type PerchId } from "./perches";
import type { PetLevel, PetPose } from "./pose";
import type { PetRoutine } from "./routines";

/**
 * A moment the pet reacts to on top of its pose: a boop (a second one
 * quickly makes it giggle), reaching a level, a helper's changes applied,
 * or a landing after a hard fling or a shake.
 */
export type PetReaction = "booped" | "giggle" | "levelUp" | "applied" | "dizzy";

/** What the pet is doing where it is. */
export type PetActivity = "ride" | "sit" | "walk" | "nap" | "watch" | "celebrate" | "tucked" | "drag";

export interface BehaviorInput {
  level: PetLevel;
  roam: boolean;
  status: Pick<LiveStatus, "kind" | "tone">;
  stage: StageView;
  /** The user is typing in the composer. */
  typing: boolean;
  dragging: boolean;
  /** A popover, menu or dialog is open. */
  overlayOpen: boolean;
  /** Since the last thing the user or the agent did. */
  idleMs: number;
  /** Perches on screen right now. */
  available: ReadonlySet<PerchId>;
  /** Where the pet is, and where an idle wander wants it next. */
  current: PerchId | null;
  wander: PerchId | null;
}

export interface Behavior {
  perch: PerchId;
  activity: PetActivity;
  size: number;
}

/** Idle this long and the pet naps where it is. */
export const NAP_MS = 3 * 60_000;
/** Perches an idle pet wanders between. */
export const WANDER_PERCHES: readonly PerchId[] = ["composer", "sidebar", "panel"];

function at(perch: PerchId, activity: PetActivity): Behavior {
  return { perch, activity, size: PERCH_SIZE[perch] };
}

/** Where the pet rests on each stage when nothing asks it elsewhere. */
function home(input: BehaviorInput): PerchId {
  const { available, stage } = input;
  if (stage === "home" && available.has("hero")) return "hero";
  if (!input.roam) return "island";
  if (stage === "inbox" && available.has("empty")) return "empty";
  if (available.has("composer")) return "composer";
  return "island";
}

/**
 * Where the pet goes and what it does, highest priority first: dragged,
 * needed (it hops into the island to point at the card), working (it rides
 * the island), watching you type, celebrating, stepping aside for an
 * overlay, napping, then wandering. At the calm level it never leaves the
 * island; with roaming off it only visits the hero spot on Home.
 */
export function petBehavior(input: BehaviorInput): Behavior | null {
  if (input.level === "off") return null;
  const { status, available } = input;
  const current = input.current && available.has(input.current) ? input.current : null;

  if (input.dragging) return at(current ?? "island", "drag");
  if (input.level === "calm" || !available.size) return at("island", status.kind === "working" ? "ride" : "sit");
  if (status.kind === "attention") return at("island", "watch");
  if (status.kind === "working" || status.kind === "retrying") return at("island", "ride");

  const rest = home(input);
  if (input.typing && input.roam && available.has("composer")) return at("composer", "watch");
  if (status.kind === "done" && status.tone === "ok") return at(current ?? rest, "celebrate");

  const roaming = input.roam && rest !== "hero";
  const wanderTo = roaming && input.wander && available.has(input.wander) ? input.wander : null;
  const perch = wanderTo ?? (roaming && current && current !== "island" ? current : rest);
  if (input.overlayOpen && perch !== "island" && perch !== "hero") return at(perch, "tucked");
  if (input.idleMs >= NAP_MS) return at(perch, "nap");
  if (perch === "island") return at("island", "sit");
  return at(perch, wanderTo && wanderTo !== current ? "walk" : "sit");
}

const REACTION_POSE: Record<PetReaction, Pick<PetPose, "mood" | "particles">> = {
  booped: { mood: "love", particles: "none" },
  giggle: { mood: "giggle", particles: "none" },
  levelUp: { mood: "excited", particles: "confetti" },
  applied: { mood: "excited", particles: "none" },
  dizzy: { mood: "dizzy", particles: "none" },
};

/** Moods a finish already shows; a celebration keeps them. */
const CHEERFUL = new Set<PetPose["mood"]>(["proud", "relieved", "happy", "excited"]);

/** An idle routine that changes the face (the rest only move the body). */
function routinePose(base: PetPose, routine: PetRoutine | null): PetPose {
  if (!routine || base.tone !== "quiet" || base.mood !== "idle") return base;
  if (routine === "yawn") return { ...base, mood: "yawning" };
  if (routine === "wave") return { ...base, mood: "greeting" };
  if (routine === "lookAround") return { ...base, gaze: "scan" };
  if (routine === "lieDown") return { ...base, mood: "sleepy", gaze: "down" };
  if (routine === "sit" || routine === "stretch") return { ...base, mood: "content" };
  return base;
}

/**
 * The face for what the pet is doing: a reaction wins, then being held,
 * napping, walking, celebrating, watching you type or peeking out from
 * where it tucked itself away, then an idle routine. Work and "needs you"
 * poses stay as they are while it rides the island.
 */
export function reactionPose(
  base: PetPose,
  activity: PetActivity,
  reaction: PetReaction | null,
  routine: PetRoutine | null = null,
): PetPose {
  if (reaction) return { ...base, ...REACTION_POSE[reaction] };
  switch (activity) {
    case "drag":
      return { ...base, mood: "surprised", gaze: "down" };
    case "nap":
      return { ...base, mood: "asleep", gaze: "down" };
    case "walk":
      return base.tone === "quiet" && base.mood === "idle" ? { ...base, mood: "content" } : base;
    case "celebrate":
      return CHEERFUL.has(base.mood) ? base : { ...base, mood: "happy", particles: "sparkles" };
    case "watch":
      return base.mood === "listening" ? { ...base, mood: "watching" } : base;
    case "tucked":
      return { ...base, mood: "peeking", gaze: "up" };
    default:
      return routinePose(base, routine);
  }
}

/** A boop while the last one still shows makes it giggle. */
export function boopReaction(current: PetReaction | null): PetReaction {
  return current === "booped" || current === "giggle" ? "giggle" : "booped";
}

/** The next edge an idle pet wanders to: another one on screen, or null to stay. */
export function pickWander(available: ReadonlySet<PerchId>, current: PerchId | null, random: number): PerchId | null {
  const options = WANDER_PERCHES.filter((p) => available.has(p) && p !== current);
  if (!options.length) return null;
  return options[Math.min(options.length - 1, Math.floor(random * options.length))] ?? null;
}
