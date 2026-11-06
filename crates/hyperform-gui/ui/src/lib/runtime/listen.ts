import { listen } from "@tauri-apps/api/event";
import type { EventEnvelope } from "../protocol/EventEnvelope";
import { runEffects } from "./effects";
import { sessions } from "./sessions.svelte";

let started: Promise<void> | null = null;

/** The one Tauri event subscription: every engine envelope for every session. */
export function initEvents(): Promise<void> {
  started ??= listen<EventEnvelope>("engineEvent", (e) => {
    runEffects(sessions.apply(e.payload));
  }).then(() => undefined);
  return started;
}
