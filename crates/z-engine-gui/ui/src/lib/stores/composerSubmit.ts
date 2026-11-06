import type { SubmissionPlan } from "../domain/composerSubmit";
import { REMEMBER_TARGETS, rememberArgs, type RememberScope } from "../domain/remember";
import { interrupt, pushToast, runCommand, runShell, steer, submitPrompt } from "../runtime";
import { composer } from "./composer.svelte";
import { ui } from "./ui.svelte";
import { runUiCommand } from "./uiCommands";

/** Clear the draft optimistically; put it back if the engine refused the command. Sending shows the chat. */
async function sendClearing(draft: string, run: () => Promise<boolean>): Promise<boolean> {
  composer.setDraft("", false);
  ui.view = "chat";
  const ok = await run();
  if (!ok && !composer.draft) composer.setDraft(draft, false);
  return ok;
}

/** Carry out what the composer decided Enter means. Returns false when nothing was sent. */
export async function executePlan(plan: SubmissionPlan): Promise<boolean> {
  const draft = composer.draft;
  switch (plan.kind) {
    case "none":
    case "remember":
      return false;
    case "submit": {
      const attachments = composer.takeAttachments();
      const ok = await sendClearing(draft, () => submitPrompt(plan.text, attachments));
      if (!ok) composer.restoreAttachments(attachments);
      return ok;
    }
    case "steer":
      if (composer.attachments.length > 0) pushToast("Attachments stay for your next prompt", "info");
      return sendClearing(draft, () => steer(plan.text));
    case "interrupt":
      return sendClearing(draft, () => interrupt(plan.text));
    case "shell":
      return sendClearing(draft, () => runShell(plan.command));
    case "command":
      return sendClearing(draft, () => runCommand(plan.name, plan.args));
    case "ui":
      composer.setDraft("", false);
      await runUiCommand(plan.name, plan.args);
      return true;
  }
}

/** `#note` -> `runCommand remember <scope> <note>`. */
export async function rememberNote(scope: RememberScope, note: string): Promise<boolean> {
  const draft = composer.draft;
  const ok = await sendClearing(draft, () => runCommand("remember", rememberArgs(scope, note)));
  const target = REMEMBER_TARGETS.find((t) => t.scope === scope);
  if (ok && target) pushToast(`Remembered · ${target.label}`, "ok");
  return ok;
}
