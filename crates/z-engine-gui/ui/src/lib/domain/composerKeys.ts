export type ComposerIntent =
  | "send"
  | "interrupt"
  | "cancel"
  | "clear"
  | "historyPrev"
  | "historyNext"
  | "togglePlan"
  | "none";

export interface ComposerKey {
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  isComposing: boolean;
}

export interface ComposerKeyState {
  busy: boolean;
  text: string;
  caret: number;
}

/**
 * Keyboard contract: Enter sends (steers while busy), Cmd/Ctrl+Enter
 * interrupts, Esc cancels the turn (or clears the draft), arrows walk history
 * from the first/last line, Shift+Tab turns plan mode on or off.
 */
export function composerIntent(e: ComposerKey, state: ComposerKeyState): ComposerIntent {
  if (e.isComposing) return "none";
  const mod = e.metaKey || e.ctrlKey;
  if (e.key === "Enter" && !e.shiftKey && !e.altKey) return mod ? "interrupt" : "send";
  if (e.key === "Escape") {
    if (state.busy) return "cancel";
    return state.text ? "clear" : "none";
  }
  if (e.key === "Tab" && e.shiftKey && !mod) return "togglePlan";
  if (mod || e.shiftKey || e.altKey) return "none";
  if (e.key === "ArrowUp" && !state.text.slice(0, state.caret).includes("\n")) return "historyPrev";
  if (e.key === "ArrowDown" && !state.text.slice(state.caret).includes("\n")) return "historyNext";
  return "none";
}
