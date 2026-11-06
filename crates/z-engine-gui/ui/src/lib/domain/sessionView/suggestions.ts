import type { Suggestion } from "../../protocol/Suggestion";
import type { SessionView } from "./types";

const MAX_SUGGESTIONS = 5;

/** A card a decision use offered (`suggested`); live only. */
export interface SuggestionView {
  suggestion: Suggestion;
  /** The turn it belongs to, as a count of turns; it hides once a later turn starts. */
  turn: number;
}

/** Turns started so far: finished ones plus the one running. */
function startedTurns(view: SessionView): number {
  return view.turns.length + (view.activeTurn ? 1 : 0);
}

/**
 * Turn-start suggestions arrive just before their turn starts and review
 * suggestions while it is still running, so both belong to the turn after
 * the finished ones.
 */
export function addSuggestion(view: SessionView, suggestion: Suggestion): SessionView {
  const id = suggestion.suggestionId;
  const rest = view.suggestions.filter((s) => s.suggestion.suggestionId !== id);
  const next = [...rest, { suggestion, turn: view.turns.length + 1 }];
  return { ...view, suggestions: next.slice(-MAX_SUGGESTIONS) };
}

export function dropSuggestion(view: SessionView, suggestionId: string): SessionView {
  const suggestions = view.suggestions.filter((s) => s.suggestion.suggestionId !== suggestionId);
  return suggestions.length === view.suggestions.length ? view : { ...view, suggestions };
}

/** The cards to show: from the current turn, and "plan first" only at the Default level outside plan mode. */
export function currentSuggestions(view: SessionView): Suggestion[] {
  const started = startedTurns(view);
  return view.suggestions
    .filter((s) => started <= s.turn)
    .map((s) => s.suggestion)
    .filter((s) => s.kind.type !== "planFirst" || (!view.planning && view.mode === "default"));
}
