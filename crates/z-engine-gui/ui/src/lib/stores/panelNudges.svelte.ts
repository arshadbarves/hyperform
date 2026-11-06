import { untrack } from "svelte";
import { EMPTY_WATCH, panelNudge, watchOf, type PanelWatch } from "../domain/sidePanel";
import { sessions } from "../runtime";
import { ui } from "./ui.svelte";

/**
 * Lets the side panel react to new work in the open chat: a plan waiting
 * for review opens the Plan tab, a helper starting badges Agents. Call it
 * once while a component that lives as long as the shell is set up.
 */
export function followPanelNudges(): void {
  let before: PanelWatch = EMPTY_WATCH;
  $effect(() => {
    const now = watchOf(sessions.active);
    untrack(() => {
      const nudge = panelNudge(before, now, ui.panelTab);
      before = now;
      if (nudge.open) ui.openPanel(nudge.open);
      ui.badgePanel(nudge.badge);
    });
  });
}
