import type { Event } from "../protocol/Event";
import type { EventEnvelope } from "../protocol/EventEnvelope";
import { sameRecord } from "../domain/sessionList";
import {
  activityMap,
  applyEnvelope,
  applyLocalEvent,
  dismissTrustRequest,
  emptySessionsState,
  eventEffects,
  forgetSession,
  markRead,
  type RuntimeEffect,
  type SessionActivity,
  type SessionsState,
  type UnreadMark,
} from "../domain/sessions";
import type { SessionView } from "../domain/sessionView";

/**
 * Every session the engine talks about, reduced from `engineEvent`
 * envelopes. Background sessions keep updating; one session is active.
 */
class SessionsStore {
  #state: SessionsState = $state.raw(emptySessionsState());
  #activity: Record<string, SessionActivity> = {};
  activeId: string | null = $state(null);

  get views(): Record<string, SessionView> {
    return this.#state.views;
  }

  get unread(): Record<string, UnreadMark> {
    return this.#state.unread;
  }

  get active(): SessionView | null {
    const id = this.activeId;
    return id ? (this.#state.views[id] ?? null) : null;
  }

  /** Active session chosen but its snapshot has not arrived yet. */
  get hydrating(): boolean {
    return this.activeId !== null && !this.#state.views[this.activeId]?.info;
  }

  /** Sidebar activity; the same object is returned while nothing changed. */
  activity: Record<string, SessionActivity> = $derived.by(() => {
    const next = activityMap(this.#state.views);
    if (!sameRecord(next, this.#activity)) this.#activity = next;
    return this.#activity;
  });

  view(sessionId: string | null | undefined): SessionView | null {
    return sessionId ? (this.#state.views[sessionId] ?? null) : null;
  }

  /** Reduce one engine envelope; returns the side effects it asks for. */
  apply(env: EventEnvelope, now = Date.now()): RuntimeEffect[] {
    const prev = this.#state;
    const next = applyEnvelope(prev, env, this.activeId, now);
    if (next === prev) return [];
    this.#state = next;
    return eventEffects(env.event, env.sessionId === this.activeId);
  }

  applyLocal(sessionId: string, event: Event, now = Date.now()) {
    this.#state = applyLocalEvent(this.#state, sessionId, event, now);
  }

  dismissTrust(sessionId: string) {
    this.#state = dismissTrustRequest(this.#state, sessionId);
  }

  activate(sessionId: string | null) {
    this.activeId = sessionId;
    if (sessionId) this.#state = markRead(this.#state, sessionId);
  }

  forget(sessionId: string) {
    this.#state = forgetSession(this.#state, sessionId);
    if (this.activeId === sessionId) this.activeId = null;
  }
}

export const sessions = new SessionsStore();
