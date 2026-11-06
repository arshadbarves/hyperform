import { revealPath } from "../commands";
import { continueTarget } from "../domain/work/homeCard";
import { moveNotice, type PickerRow } from "../domain/work/picker";
import type { PlanSummary } from "../protocol/PlanSummary";
import { attachPlanHere, chatTitle, ensureSession, errorText, pushToast, sessionList, takePlanInNewChat } from "../runtime";
import { resetShell } from "../shellStore";
import { openChat } from "./app-actions";
import { composer } from "./composer.svelte";
import { confirmStore } from "./confirm.svelte";
import { ui } from "./ui.svelte";

/**
 * Continue a plan in this chat from the picker (the Plan tab, the
 * composer's + menu, the palette). A plan another chat has moves here
 * once you confirm.
 */
export async function pickPlan(row: PickerRow): Promise<boolean> {
  if (row.elsewhere) {
    const notice = moveNotice(row.plan.goal, chatTitle(row.elsewhere));
    const ok = await confirmStore.ask({ ...notice, confirmLabel: "Move it here", tone: "default" });
    if (!ok) return false;
  }
  if (!(await attachPlanHere(row.plan.planId))) return false;
  ui.view = "chat";
  ui.openPlan(row.plan.planId);
  return true;
}

/**
 * Open a plan from the project home or the Inbox, on its Plan tab: in the
 * chat it was last in while that chat exists, else in a new chat that
 * takes it. `tell`: the new chat is also told to continue (the home's
 * Continue), unless the plan is paused.
 */
export async function openWorkPlan(plan: PlanSummary, tell: boolean): Promise<boolean> {
  const target = continueTarget(plan, sessionList.items);
  if (target.kind === "open") {
    await openChat(target.sessionId, plan.projectRoot);
    ui.openPlan(plan.planId);
    return true;
  }
  const id = await takePlanInNewChat(plan.planId, plan.projectRoot, tell ? target.prompt : null);
  if (!id) return false;
  ui.view = "chat";
  composer.clear();
  resetShell();
  ui.openPlan(plan.planId);
  return true;
}

/** "Continue a plan…" (the composer's + menu, the palette): the Plan tab listing the project's plans for this chat, a new one when none is open. */
export async function showPlanPicker(): Promise<void> {
  const id = await ensureSession();
  if (!id) return;
  ui.view = "chat";
  ui.openPlanPicker(id);
}

/** Show a helper's kept worktree in the file manager. */
export async function revealWorktree(path: string): Promise<void> {
  try {
    await revealPath(path);
  } catch (e) {
    pushToast(`Could not show the worktree · ${errorText(e)}`, "warn");
  }
}
