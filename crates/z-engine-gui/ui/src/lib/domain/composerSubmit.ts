import type { SlashCommandInfo } from "../commands/engine";
import type { Attachment } from "../protocol/Attachment";
import { findCommand, parseSlash } from "./slashCommands";

export type SubmissionPlan =
  | { kind: "none" }
  | { kind: "submit"; text: string; attachments: Attachment[] }
  | { kind: "steer"; text: string }
  | { kind: "interrupt"; text: string | null }
  | { kind: "shell"; command: string }
  | { kind: "command"; name: string; args: string }
  | { kind: "ui"; name: string; args: string }
  | { kind: "remember"; text: string };

export interface SubmissionInput {
  text: string;
  attachments: Attachment[];
  busy: boolean;
  /** Cmd/Ctrl+Enter: stop the current round and send this right after. */
  interrupt: boolean;
  commands: SlashCommandInfo[];
}

/** `#note` at the very start is a memory, `##` is a markdown heading. */
export function isRememberText(text: string): boolean {
  return /^#(?!#)/.test(text.trimStart());
}

/** Decide what Enter does with the composer contents. */
export function planSubmission(input: SubmissionInput): SubmissionPlan {
  const text = input.text.trim();
  if (input.interrupt && input.busy) return { kind: "interrupt", text: text || null };
  if (!text) {
    return input.attachments.length > 0 && !input.busy
      ? { kind: "submit", text: "", attachments: input.attachments }
      : { kind: "none" };
  }
  if (text.startsWith("!")) {
    const command = text.slice(1).trim();
    return command ? { kind: "shell", command } : { kind: "none" };
  }
  if (isRememberText(text)) {
    const note = text.replace(/^#/, "").trim();
    return note ? { kind: "remember", text: note } : { kind: "none" };
  }
  const slash = parseSlash(text);
  const command = slash ? findCommand(input.commands, slash.name) : null;
  if (slash && command) {
    return command.kind === "ui"
      ? { kind: "ui", name: slash.name, args: slash.args }
      : { kind: "command", name: slash.name, args: slash.args };
  }
  if (input.busy) return { kind: "steer", text };
  return { kind: "submit", text, attachments: input.attachments };
}

/** Whether a plan consumes the composer's attachments. */
export function consumesAttachments(plan: SubmissionPlan): boolean {
  return plan.kind === "submit";
}
