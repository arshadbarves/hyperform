import { listSessions } from "../commands";
import { listItems, reuseItems, type SessionListItem } from "../domain/sessionList";
import type { SessionSummary } from "../protocol/SessionSummary";
import { sessions } from "./sessions.svelte";

/** `list_sessions` mirror plus the merged rows the sidebar and palette render. */
class SessionListStore {
  summaries: SessionSummary[] = $state.raw([]);
  loaded = $state(false);
  #items: SessionListItem[] = [];
  #timer: ReturnType<typeof setTimeout> | null = null;

  items: SessionListItem[] = $derived.by(() => {
    this.#items = reuseItems(this.#items, listItems(this.summaries, sessions.views));
    return this.#items;
  });

  async refresh() {
    try {
      this.summaries = await listSessions();
      this.loaded = true;
    } catch (e) {
      console.error("list_sessions failed", e);
    }
  }

  /** Coalesce bursts (title + turn events) into one listing. */
  refreshSoon(delayMs = 400) {
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => {
      this.#timer = null;
      void this.refresh();
    }, delayMs);
  }

  remove(sessionId: string) {
    this.summaries = this.summaries.filter((s) => s.sessionId !== sessionId);
  }
}

export const sessionList = new SessionListStore();
