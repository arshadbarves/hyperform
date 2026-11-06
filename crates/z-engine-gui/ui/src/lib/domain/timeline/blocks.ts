import type { ContentBlock } from "../../protocol/ContentBlock";
import type { MediaSource } from "../../protocol/MediaSource";
import type { Message } from "../../protocol/Message";
import type { ToolResultPart } from "../../protocol/ToolResultPart";
import type { JsonValue } from "../../protocol/serde_json/JsonValue";

/** A `tool_use` block lifted out of an assistant message. */
export interface ToolUseRef {
  callId: string;
  name: string;
  input: JsonValue;
}

export interface ToolResultInfo {
  text: string;
  images: MediaSource[];
  isError: boolean;
}

const REMINDER = /^\s*<system-reminder>[\s\S]*<\/system-reminder>\s*$/;

/** A prompt command's expanded body: `<command name="...">body</command>` (groups: name, body). */
export const COMMAND_WRAPPER = /^\s*<command name="([^"]+)">\n?([\s\S]*?)\n?<\/command>\s*$/;

/** Engine reminders ride along in user messages; they are model context, not chat. */
export function isReminderText(text: string): boolean {
  return REMINDER.test(text);
}

export function hasToolResults(message: Message): boolean {
  return message.content.some((block) => block.type === "toolResult");
}

/** Text the user typed (reminders, command bodies and tool payloads excluded). */
export function visibleText(message: Message): string {
  const parts: string[] = [];
  for (const block of message.content) {
    if (
      block.type === "text" &&
      block.text.trim() &&
      !isReminderText(block.text) &&
      !COMMAND_WRAPPER.test(block.text)
    ) {
      parts.push(block.text);
    }
  }
  return parts.join("\n");
}

export function messageImages(message: Message): MediaSource[] {
  const out: MediaSource[] = [];
  for (const block of message.content) {
    if (block.type === "image") out.push(block.source);
  }
  return out;
}

export function messageDocuments(message: Message): Array<{ title: string | null }> {
  const out: Array<{ title: string | null }> = [];
  for (const block of message.content) {
    if (block.type === "document") out.push({ title: block.title });
  }
  return out;
}

/** True when a user message carries something worth a card. */
export function hasVisibleUserContent(message: Message): boolean {
  return (
    visibleText(message).length > 0 ||
    message.content.some((b) => b.type === "image" || b.type === "document")
  );
}

/** Id of the newest user message a person typed (tool-result rounds excluded). */
export function lastPromptId(messages: Message[] | undefined): string | null {
  if (!messages) return null;
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (m.role === "user" && !hasToolResults(m) && hasVisibleUserContent(m)) return m.id;
  }
  return null;
}

export function toolUses(message: Message): ToolUseRef[] {
  const out: ToolUseRef[] = [];
  for (const block of message.content) {
    if (block.type === "toolUse") out.push({ callId: block.id, name: block.name, input: block.input });
  }
  return out;
}

export function toolUseIds(messages: Message[]): Set<string> {
  const ids = new Set<string>();
  for (const message of messages) {
    for (const block of message.content) {
      if (block.type === "toolUse") ids.add(block.id);
    }
  }
  return ids;
}

export function resultText(parts: ToolResultPart[]): string {
  return parts
    .filter((part): part is Extract<ToolResultPart, { type: "text" }> => part.type === "text")
    .map((part) => part.text)
    .join("\n");
}

/** Every `tool_result` in the transcript, keyed by the call it answers. */
export function pairResults(messages: Message[]): Record<string, ToolResultInfo> {
  const out: Record<string, ToolResultInfo> = {};
  for (const message of messages) {
    for (const block of message.content) {
      if (block.type !== "toolResult") continue;
      out[block.toolUseId] = {
        text: resultText(block.content),
        images: block.content.flatMap((part) => (part.type === "image" ? [part.source] : [])),
        isError: block.isError,
      };
    }
  }
  return out;
}

export function mediaSrc(source: MediaSource): string {
  return source.kind === "base64" ? `data:${source.mediaType};base64,${source.data}` : source.url;
}

export function isTextBlock(block: ContentBlock): block is Extract<ContentBlock, { type: "text" }> {
  return block.type === "text";
}
