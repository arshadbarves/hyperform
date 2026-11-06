import type { Message } from "../../protocol/Message";
import { COMMAND_WRAPPER, isReminderText } from "./blocks";

/** A user message sent by a prompt command: `/name args` plus its expanded body. */
export interface CommandChip {
  name: string;
  args: string;
  /** What the user typed, e.g. `/review src/`. */
  invocation: string;
  /** The expanded prompt the model received (without the wrapper tags). */
  body: string;
}

const INVOCATION = /^\/(\S+)(?:\s+([\s\S]*))?$/;

/** The chip for a command message: its second text block is a `<command name=...>` wrapper. */
export function commandChip(message: Message): CommandChip | null {
  if (message.role !== "user") return null;
  const texts: string[] = [];
  for (const block of message.content) {
    if (block.type === "text" && !isReminderText(block.text)) texts.push(block.text);
  }
  if (texts.length < 2) return null;
  const wrapper = COMMAND_WRAPPER.exec(texts[1]);
  const invocation = INVOCATION.exec(texts[0].trim());
  if (!wrapper || !invocation || invocation[1] !== wrapper[1]) return null;
  return {
    name: wrapper[1],
    args: (invocation[2] ?? "").trim(),
    invocation: texts[0].trim(),
    body: wrapper[2].trim(),
  };
}
