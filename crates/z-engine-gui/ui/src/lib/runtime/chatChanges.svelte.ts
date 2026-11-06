import { sessionChangedFiles } from "../commands";

/** Files changed since each chat's first checkpoint, counted for the title bar's Changes button. */
class ChatChangesStore {
  #counts = $state.raw<Record<string, number>>({});

  count(sessionId: string | null): number {
    return sessionId ? (this.#counts[sessionId] ?? 0) : 0;
  }

  async refresh(sessionId: string): Promise<void> {
    try {
      const files = await sessionChangedFiles(sessionId);
      if (this.#counts[sessionId] !== files.length) this.#counts = { ...this.#counts, [sessionId]: files.length };
    } catch {
      // No checkpoints (no git, or a chat that has not started): nothing to count.
    }
  }
}

export const chatChanges = new ChatChangesStore();
