import { helpMarkdown } from "../domain/helpText";
import { modLabel } from "../platform";
import {
  activeProjectRoot,
  catalogs,
  ensureSession,
  exportTranscript,
  runCommand,
  sessions,
} from "../runtime";
import { openSettings, startNewChat } from "./app-actions";
import { composer } from "./composer.svelte";
import { island } from "./island.svelte";
import { ui } from "./ui.svelte";

function hookReport(): string {
  const hooks = sessions.active?.hooks ?? [];
  if (hooks.length === 0) {
    return "No hooks have run in this chat. Configure them in settings as `[[hooks.<Event>]]` with `matcher`, `command` and `timeout_secs`.";
  }
  const rows = hooks
    .slice(-12)
    .map((h) => `- **${h.hookEvent}** \`${h.command}\`${h.blocked ? " — blocked" : ""}${h.message ? `: ${h.message}` : ""}`);
  return ["**Recent hook runs**", ...rows].join("\n");
}

async function showLocal(name: string, markdown: string) {
  const id = await ensureSession();
  if (id) sessions.applyLocal(id, { type: "commandOutput", name, markdown });
}

/** GUI side of `kind: "ui"` slash commands. Unknown names go to the engine. */
export async function runUiCommand(name: string, args: string): Promise<void> {
  switch (name) {
    case "help":
      return showLocal("help", helpMarkdown(catalogs.commandsFor(activeProjectRoot()), modLabel()));
    case "agents":
      return ui.openPanel("agents");
    case "jobs":
      return ui.openJobs();
    case "permissions":
      return openSettings("permissions");
    case "config":
      return openSettings("models");
    case "hooks":
      return showLocal("hooks", hookReport());
    case "memory":
      return composer.setDraft("# ");
    case "resume":
      return ui.openPalette(true);
    case "export":
      return exportTranscript(args.trim() === "json" ? "json" : "markdown");
    case "clear":
      return startNewChat();
    case "context":
      if (sessions.active?.info) island.contextOpen = true;
      else await runCommand("context", args);
      return;
    default:
      await runCommand(name, args);
  }
}
