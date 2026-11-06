import type { CompanionLevel } from "../../protocol/config/CompanionLevel";
import type { TurnRecord } from "../../protocol/TurnRecord";
import type { TurnTone } from "../../protocol/TurnTone";
import type { LiveActivity, LiveStatus, LiveTone } from "../liveStatus";
import { MAIN_AGENT, type SessionView } from "../sessionView/types";
import { latestTurnTone, toneMood } from "./decisionMood";

/** How much the pet reacts (`ui.companion`). */
export type PetLevel = CompanionLevel;

/** Every mood the pet can be in; `emotions.ts` says how each one looks. */
export type PetMood =
  // the agent's work
  | "thinking"
  | "reading"
  | "searching"
  | "coding"
  | "talking"
  | "busy"
  | "planning"
  | "tidying"
  | "determined"
  // it needs you
  | "asking"
  | "pleading"
  | "presenting"
  // how it turned out
  | "proud"
  | "relieved"
  | "excited"
  | "happy"
  | "content"
  | "sad"
  | "worried"
  | "confused"
  // you, and idle time
  | "surprised"
  | "dizzy"
  | "love"
  | "giggle"
  | "greeting"
  | "listening"
  | "watching"
  | "curious"
  | "peeking"
  | "bored"
  | "yawning"
  | "sleepy"
  | "asleep"
  | "idle";

export type Gaze = "center" | "down" | "up" | "left" | "right" | "scan" | "pointer";
/** Extras on top of a mood's own bubble and bursts (see `expression` in emotions.ts). */
export type Particles = "none" | "thought" | "sparkles" | "sleep" | "hearts" | "confetti";

/** What the user is doing, from DOM events; see `lib/stores/userSignals.svelte.ts`. */
export interface UserSignals {
  typingAt: number | null;
  scrolledBack: boolean;
  hovering: boolean;
  lastActiveAt: number;
  returnedAt: number | null;
}

export interface PetPose {
  mood: PetMood;
  gaze: Gaze;
  particles: Particles;
  tone: LiveTone;
}

/** What the status line does not say but the pet reacts to, from the chat (`poseContext`). */
export interface PoseContext {
  /** When the running turn started; null while idle. */
  turnStartedAt: number | null;
  /** The turn before the last one failed, so passing now is a relief. */
  afterFailure: boolean;
  /** When a main-agent tool call in this turn last failed. */
  toolErrorAt: number | null;
  /** When the agent last finished working. */
  workedAt: number | null;
  /** How the decision model judged the latest finished turn (`decisions_pet_mood`). */
  turnTone: TurnTone | null;
}

/** Keystrokes count as "listening" this long after the last one. */
export const TYPING_MS = 1600;
/** Nothing from the user or the agent for this long, and the pet gets bored. */
export const BORED_MS = 75_000;
/** Longer still, and it dozes. */
export const SLEEPY_MS = 3 * 60_000;
/** And then it falls asleep. */
export const ASLEEP_MS = 6 * 60_000;
/** How long the wave lasts after the user comes back. */
export const GREETING_MS = 4000;
/** A turn running longer than this makes the pet determined. */
export const LONG_TASK_MS = 2 * 60_000;
/** A failed tool call leaves it confused this long. */
export const TOOL_ERROR_MS = 4000;

export const NO_CONTEXT: PoseContext = {
  turnStartedAt: null,
  afterFailure: false,
  toolErrorAt: null,
  workedAt: null,
  turnTone: null,
};

export function pose(mood: PetMood, gaze: Gaze, tone: LiveTone, particles: Particles = "none"): PetPose {
  return { mood, gaze, particles, tone };
}

/** The pet's mood and gaze for each kind of work, so every tool reads differently. */
const WORK: Record<LiveActivity, [PetMood, Gaze]> = {
  think: ["thinking", "up"],
  reply: ["talking", "down"],
  compact: ["tidying", "down"],
  read: ["reading", "scan"],
  search: ["searching", "scan"],
  web: ["searching", "scan"],
  verify: ["searching", "scan"],
  edit: ["coding", "down"],
  write: ["coding", "down"],
  todo: ["planning", "up"],
  plan: ["presenting", "center"],
  question: ["asking", "center"],
  approval: ["pleading", "center"],
  bash: ["busy", "center"],
  job: ["busy", "center"],
  mcp: ["busy", "center"],
  agent: ["busy", "up"],
  generic: ["busy", "center"],
  warn: ["worried", "center"],
  retry: ["worried", "center"],
  done: ["content", "center"],
  failed: ["sad", "down"],
  info: ["idle", "center"],
};

/** Plain effort turns into determination on a long task; tools keep their props. */
const TIRELESS = new Set<PetMood>(["thinking", "busy", "coding", "talking"]);

/** Which kind of "needs you" it is. */
const NEEDS: Partial<Record<LiveActivity, PetMood>> = { question: "asking", approval: "pleading", plan: "presenting" };

function workPose(status: LiveStatus, ctx: PoseContext, now: number): PetPose {
  const [mood, gaze] = WORK[status.activity ?? "think"];
  if (ctx.toolErrorAt !== null && now - ctx.toolErrorAt < TOOL_ERROR_MS && mood !== "tidying") return pose("confused", "center", "working");
  const long = ctx.turnStartedAt !== null && now - ctx.turnStartedAt > LONG_TASK_MS;
  return pose(long && TIRELESS.has(mood) ? "determined" : mood, gaze, "working");
}

function outcomePose(status: LiveStatus, ctx: PoseContext): PetPose | null {
  const done = status.kind === "done";
  const judged = done ? toneMood(ctx.turnTone, status.tone) : null;
  if (judged) return pose(judged, "center", status.tone);
  if (status.tone === "ok") return pose(done ? (ctx.afterFailure ? "relieved" : "proud") : "happy", "center", "ok");
  if (status.tone === "danger") return pose("sad", "down", "danger");
  if (status.tone === "attention") return pose("worried", "center", "attention");
  return done ? pose("content", "center", "quiet") : null;
}

function agentPose(status: LiveStatus, ctx: PoseContext, now: number): PetPose | null {
  switch (status.kind) {
    case "attention":
      return pose((status.activity && NEEDS[status.activity]) || "pleading", "center", "attention");
    case "retrying":
      return pose("worried", "left", "attention");
    case "working":
      return workPose(status, ctx, now);
    case "notice":
    case "done":
      return outcomePose(status, ctx);
    default:
      return null;
  }
}

function userPose(signals: UserSignals, ctx: PoseContext, now: number): PetPose | null {
  if (signals.returnedAt !== null && now - signals.returnedAt < GREETING_MS) return pose("greeting", "center", "quiet");
  if (signals.typingAt !== null && now - signals.typingAt < TYPING_MS) return pose("listening", "down", "quiet");
  if (signals.scrolledBack) return pose("curious", "up", "quiet");
  if (signals.hovering) return null;
  const idle = now - Math.max(signals.lastActiveAt, ctx.workedAt ?? 0);
  if (idle > ASLEEP_MS) return pose("asleep", "down", "quiet");
  if (idle > SLEEPY_MS) return pose("sleepy", "down", "quiet");
  if (idle > BORED_MS) return pose("bored", "left", "quiet");
  return null;
}

/** Moods whose eyes stay where they are, whatever the user does. */
const OWN_GAZE = new Set<PetMood>(["sleepy", "asleep", "greeting", "bored"]);

/**
 * The pet's body language. The agent's state always wins; at the lively
 * level the eyes still follow what the user is doing (the pointer, the
 * composer while typing), and an idle agent reacts to the user.
 */
export function petPose(
  status: LiveStatus,
  signals: UserSignals,
  level: PetLevel,
  now: number,
  ctx: PoseContext = NO_CONTEXT,
): PetPose | null {
  if (level === "off") return null;
  const lively = level === "lively";
  const base =
    agentPose(status, ctx, now) ??
    (lively ? userPose(signals, ctx, now) : null) ??
    (status.waiting ? pose("curious", "left", "attention") : pose("idle", "center", "quiet"));
  if (!lively || OWN_GAZE.has(base.mood)) return base;
  if (signals.hovering) return { ...base, gaze: "pointer" };
  if (signals.typingAt !== null && now - signals.typingAt < TYPING_MS) return { ...base, gaze: "down" };
  return base;
}

function failed(turn: TurnRecord | undefined): boolean {
  return !!turn && (turn.verification.status === "failed" || turn.outcome.type === "failed");
}

/** The chat facts the pet reacts to beyond the status line. */
export function poseContext(view: SessionView | null): PoseContext {
  if (!view) return NO_CONTEXT;
  const turns = view.turns;
  const started = view.activeTurn?.startedAt ?? null;
  let toolErrorAt: number | null = null;
  if (started !== null) {
    for (const tool of Object.values(view.tools)) {
      if (tool.agentId !== MAIN_AGENT || tool.status !== "error" || tool.startedAt < started) continue;
      toolErrorAt = Math.max(toolErrorAt ?? 0, tool.startedAt + (tool.durationMs ?? 0));
    }
  }
  return {
    turnStartedAt: started,
    afterFailure: failed(turns[turns.length - 2]),
    toolErrorAt,
    workedAt: turns[turns.length - 1]?.finishedAt ?? null,
    turnTone: latestTurnTone(view),
  };
}
