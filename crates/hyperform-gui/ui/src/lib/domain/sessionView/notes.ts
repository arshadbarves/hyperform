import type { NoticeLevel } from "../../protocol/NoticeLevel";
import { capped, lastMessageId } from "./records";
import type { SessionView } from "./types";

const MAX_NOTICES = 50;
const MAX_HOOKS = 30;
const MAX_OUTPUTS = 100;
const MAX_ERRORS = 50;

export function addNotice(view: SessionView, level: NoticeLevel, text: string, now: number): SessionView {
  const id = view.nextLocalId;
  return {
    ...view,
    notices: capped(view.notices, { id, level, text, at: now }, MAX_NOTICES),
    nextLocalId: id + 1,
  };
}

export function addHookRun(
  view: SessionView,
  hook: { hookEvent: string; command: string; blocked: boolean; message: string | null },
  now: number,
): SessionView {
  const id = view.nextLocalId;
  return { ...view, hooks: capped(view.hooks, { id, ...hook, at: now }, MAX_HOOKS), nextLocalId: id + 1 };
}

export function addCommandOutput(
  view: SessionView,
  name: string,
  markdown: string,
  now: number,
): SessionView {
  const id = view.nextLocalId;
  const output = { id, name, markdown, afterMessageId: lastMessageId(view.messages), at: now };
  return { ...view, outputs: capped(view.outputs, output, MAX_OUTPUTS), nextLocalId: id + 1 };
}

export function addError(view: SessionView, message: string, now: number): SessionView {
  const id = view.nextLocalId;
  const error = { id, message, afterMessageId: lastMessageId(view.messages), at: now };
  return {
    ...view,
    errors: capped(view.errors, error, MAX_ERRORS),
    lastError: message,
    retrying: null,
    nextLocalId: id + 1,
  };
}
