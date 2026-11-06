/**
 * Up/Down prompt history. Walking back saves the unsent draft; walking past
 * the newest entry restores it.
 */
export function createComposerHistory(max = 100) {
  let items: string[] = [];
  let pos: number | null = null;
  let saved = "";

  return {
    push(item: string) {
      if (!item.trim()) return;
      if (items[items.length - 1] !== item) items = [...items, item].slice(-max);
      pos = null;
      saved = "";
    },
    /** Older entry, or null when there is nothing older. */
    prev(current: string): string | null {
      if (items.length === 0) return null;
      if (pos === null) {
        saved = current;
        pos = items.length - 1;
      } else if (pos > 0) {
        pos -= 1;
      } else {
        return null;
      }
      return items[pos];
    },
    /** Newer entry, the saved draft past the end, or null when not navigating. */
    next(): string | null {
      if (pos === null) return null;
      if (pos + 1 >= items.length) {
        pos = null;
        return saved;
      }
      pos += 1;
      return items[pos];
    },
    reset() {
      pos = null;
    },
  };
}

export type ComposerHistory = ReturnType<typeof createComposerHistory>;
