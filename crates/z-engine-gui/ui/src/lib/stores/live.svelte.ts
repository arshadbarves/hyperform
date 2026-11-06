import { untrack } from "svelte";
import { freshFinish, liveStatus, waitingChats, type LiveStatus } from "../domain/liveStatus";
import { petPose, poseContext, type PetLevel, type PetPose } from "../domain/pet/pose";
import type { SessionView } from "../domain/sessionView";
import { chatTitle, sessions, toastStore, type Toast } from "../runtime";
import { bindStore } from "../svelte/bind.svelte";
import { ticker } from "../ui/ticker.svelte";
import { settingsStore } from "./settings.svelte";
import { userSignals } from "./userSignals.svelte";

const RECAP_MS = 8000;

export interface Live {
  readonly view: SessionView | null;
  readonly status: LiveStatus;
  /** The pet's body language; null when `ui.companion` is off. */
  readonly pose: PetPose | null;
  readonly level: PetLevel;
  readonly notice: Toast | null;
  readonly now: number;
}

/**
 * What is happening right now, computed once for the window: the status the
 * island says (`liveStatus`), the pet's pose, and the clock they tick on.
 * The title bar and the pet read it. Call during component init (AppShell).
 */
export function createLive(): Live {
  const toasts = bindStore(toastStore);
  const view = $derived(sessions.active);
  const level = $derived<PetLevel>(settingsStore.settings?.ui.companion ?? "lively");
  const running = $derived(view !== null && (view.status !== "idle" || view.retrying !== null));
  const clock = ticker(() => running || level === "lively", 500);

  let flashTurnId = $state<string | null>(null);
  const finishKey = $derived(view?.status === "idle" ? (view.turns[view.turns.length - 1]?.turnId ?? null) : null);
  $effect(() => {
    if (!finishKey) return;
    const fresh = untrack(() => freshFinish(view, Date.now()));
    if (!fresh) return;
    flashTurnId = fresh.turnId;
    const timer = window.setTimeout(() => (flashTurnId = null), fresh.remainingMs);
    return () => {
      window.clearTimeout(timer);
      flashTurnId = null;
    };
  });

  $effect(() => {
    if (userSignals.awaySince === null) return;
    const timer = window.setTimeout(() => userSignals.dismissRecap(), RECAP_MS);
    return () => window.clearTimeout(timer);
  });

  const waiting = $derived(waitingChats(sessions.activity, sessions.activeId, chatTitle));
  const notice = $derived(toasts.current[toasts.current.length - 1] ?? null);
  const status = $derived(
    liveStatus({ view, waiting, notice, flashTurnId, awaySince: userSignals.awaySince, now: clock.now }),
  );
  const context = $derived(poseContext(view));
  const pose = $derived(petPose(status, userSignals, level, clock.now, context));

  return {
    get view() {
      return view;
    },
    get status() {
      return status;
    },
    get pose() {
      return pose;
    },
    get level() {
      return level;
    },
    get notice() {
      return notice;
    },
    get now() {
      return clock.now;
    },
  };
}
