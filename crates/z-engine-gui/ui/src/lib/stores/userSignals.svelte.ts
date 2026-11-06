import type { UserSignals } from "../domain/pet/pose";

/** Leaving the window this long or more counts as being away. */
export const AWAY_MS = 5 * 60_000;
const ACTIVITY_THROTTLE_MS = 1000;
const ACTIVITY_EVENTS = ["pointerdown", "pointermove", "keydown", "wheel"] as const;

/** What the user is doing, read from DOM events only, so the companion can react to it. */
class UserSignalsStore implements UserSignals {
  typingAt = $state<number | null>(null);
  scrolledBack = $state(false);
  hovering = $state(false);
  lastActiveAt = $state(Date.now());
  returnedAt = $state<number | null>(null);
  /** When the user left, while the "while you were away" recap is showing. */
  awaySince = $state<number | null>(null);
  #leftAt: number | null = null;
  #activeMark = 0;

  typed() {
    const now = Date.now();
    this.typingAt = now;
    this.lastActiveAt = now;
  }

  dismissRecap() {
    this.awaySince = null;
    this.returnedAt = null;
  }

  /** The companion waves hello (as on return), without a recap. */
  greet() {
    this.returnedAt = Date.now();
  }

  #active(force = false) {
    const now = Date.now();
    if (!force && now - this.#activeMark < ACTIVITY_THROTTLE_MS) return;
    this.#activeMark = now;
    this.lastActiveAt = now;
  }

  #left() {
    this.#leftAt ??= Date.now();
  }

  #back() {
    const leftAt = this.#leftAt;
    this.#leftAt = null;
    this.#active(true);
    if (leftAt === null || Date.now() - leftAt < AWAY_MS) return;
    this.awaySince = leftAt;
    this.returnedAt = Date.now();
  }

  /** Listen for activity and for leaving and returning to the window; returns the cleanup. */
  track(): () => void {
    const onActive = () => this.#active();
    const onLeave = () => this.#left();
    const onBack = () => this.#back();
    const onVisibility = () => (document.hidden ? this.#left() : this.#back());
    for (const name of ACTIVITY_EVENTS) window.addEventListener(name, onActive, { capture: true, passive: true });
    window.addEventListener("blur", onLeave);
    window.addEventListener("focus", onBack);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      for (const name of ACTIVITY_EVENTS) window.removeEventListener(name, onActive, { capture: true });
      window.removeEventListener("blur", onLeave);
      window.removeEventListener("focus", onBack);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }
}

export const userSignals = new UserSignalsStore();
