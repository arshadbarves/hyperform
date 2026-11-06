/** App-wide ⌘/Ctrl shortcuts (the keyboard map in docs/design/gui-ui-guide.md). */
export type ShortcutAction = "palette" | "newChat" | "toggleSidebar" | "toggleChanges" | "settings";

export interface KeyInput {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

const KEYS: Record<string, ShortcutAction> = {
  k: "palette",
  n: "newChat",
  b: "toggleSidebar",
  d: "toggleChanges",
  ",": "settings",
};

/** The action a key chord asks for; only plain ⌘ or Ctrl chords count. */
export function shortcutFor(e: KeyInput): ShortcutAction | null {
  if (!(e.metaKey || e.ctrlKey) || e.shiftKey || e.altKey) return null;
  return KEYS[e.key.toLowerCase()] ?? null;
}
