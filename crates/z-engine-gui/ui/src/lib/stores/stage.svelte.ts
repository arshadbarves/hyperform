import { hasChatContent, stageFor, type StageView } from "../domain/stage";
import { sessions } from "../runtime";
import { ui } from "./ui.svelte";

/** What the main stage shows right now; reactive when read inside `$derived`. */
export function currentStage(): StageView {
  const active = sessions.activeId
    ? { hasContent: hasChatContent(sessions.active), hydrating: sessions.hydrating }
    : null;
  return stageFor(ui.view, active);
}
