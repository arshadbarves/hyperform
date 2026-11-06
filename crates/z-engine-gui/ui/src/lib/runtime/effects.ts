import type { RuntimeEffect } from "../domain/sessions";
import { appendShellOutput } from "../shellStore";
import { pet } from "./pet.svelte";
import { projectPlans } from "./plans.svelte";
import { sessionList } from "./sessionList.svelte";
import { pushToast } from "./toasts";

export function runEffects(effects: RuntimeEffect[]) {
  for (const effect of effects) {
    switch (effect.kind) {
      case "toast":
        pushToast(effect.text, effect.tone);
        break;
      case "shellOutput":
        appendShellOutput(effect.text);
        break;
      case "refreshSessions":
        sessionList.refreshSoon();
        break;
      case "refreshPlans":
        projectPlans.refreshSoon(effect.projectRoot);
        break;
      case "petGrowth":
        pet.record(effect.signal);
        break;
    }
  }
}
