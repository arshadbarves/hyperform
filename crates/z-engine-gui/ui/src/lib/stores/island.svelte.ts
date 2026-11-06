import { untrack } from "svelte";

/** Title-bar island chrome: whether the island or the context card is open, and when each chat's activity was last looked at. */
class IslandStore {
  open = $state(false);
  contextOpen = $state(false);
  #seen = $state.raw<Record<string, number>>({});

  seenAt(sessionId: string | null): number {
    return sessionId ? (this.#seen[sessionId] ?? 0) : 0;
  }

  /** Safe inside an effect: it must not subscribe to what it writes. */
  markSeen(sessionId: string | null, at = Date.now()) {
    if (sessionId) this.#seen = { ...untrack(() => this.#seen), [sessionId]: at };
  }
}

export const island = new IslandStore();
