import { sessionLabel, viewTitle } from "../domain/sessionList";
import { sessionList } from "./sessionList.svelte";
import { sessions } from "./sessions.svelte";

/** A chat's display title from its live view or the saved list; reactive inside `$derived`. */
export function chatTitle(sessionId: string): string {
  const summary = sessionList.summaries.find((s) => s.sessionId === sessionId);
  return sessionLabel(viewTitle(sessions.view(sessionId) ?? undefined, summary));
}
