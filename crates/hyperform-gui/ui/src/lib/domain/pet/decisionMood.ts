import type { TurnTone } from "../../protocol/TurnTone";
import type { LiveTone } from "../liveStatus";
import type { SessionView } from "../sessionView/types";
import type { PetMood } from "./pose";

/** The tone the decision model gave the latest finished turn (`decisions_pet_mood`), if any. */
export function latestTurnTone(view: SessionView | null): TurnTone | null {
  if (!view) return null;
  const last = view.turns[view.turns.length - 1];
  return (last && view.turnTones[last.turnId]) ?? null;
}

/**
 * How a judged tone colors the mood of a finished turn. It never contradicts
 * the outcome: a failed turn keeps its sad face, a passing one never looks
 * stuck. `null` keeps the event-based mood.
 */
const TONE_MOODS: Record<TurnTone, Partial<Record<LiveTone, PetMood>>> = {
  done_well: { ok: "proud", quiet: "proud" },
  smooth: { ok: "happy", quiet: "content" },
  struggling: { ok: "relieved", quiet: "relieved", attention: "worried" },
  blocked: { quiet: "confused", attention: "worried" },
};

export function toneMood(tone: TurnTone | null, outcome: LiveTone): PetMood | null {
  return (tone && TONE_MOODS[tone][outcome]) ?? null;
}
