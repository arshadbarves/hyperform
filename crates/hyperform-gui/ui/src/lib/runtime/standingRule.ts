import { listInstructionFiles, writeInstructionFile } from "../commands";
import { instructionSlots, isTruncated } from "../domain/settings/instructions";
import { appendRule, hasRule } from "../domain/settings/standingRule";
import { errorText, pushToast } from "./toasts";

/** `project` is the shared AGENTS.md, `local` the personal AGENTS.local.md. */
export type RuleScope = "project" | "local";

/**
 * Appends `rule` to one of the project's instruction files through the
 * Memory tab's write path (which reloads the settings). False when nothing
 * was saved.
 */
export async function saveStandingRule(projectRoot: string, scope: RuleScope, rule: string): Promise<boolean> {
  const name = scope === "project" ? "AGENTS.md" : "AGENTS.local.md";
  try {
    const files = await listInstructionFiles(projectRoot);
    const slot = instructionSlots(null, projectRoot, files).find((s) => s.scope === scope);
    if (!slot) return false;
    const content = slot.file?.content ?? "";
    if (isTruncated(content)) {
      pushToast(`${name} is larger than 64 KiB · edit it in Settings › Memory`, "warn");
      return false;
    }
    if (!hasRule(content, rule)) await writeInstructionFile(slot.path, appendRule(content, rule));
    pushToast(`Saved to ${name}`, "ok");
    return true;
  } catch (e) {
    pushToast(`Could not save to ${name} · ${errorText(e)}`, "warn");
    return false;
  }
}
