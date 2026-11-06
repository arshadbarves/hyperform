/**
 * The prompt inspector reads `inspect_request`, the last model request as
 * untyped JSON. Accept the engine's camelCase request (`system` blocks,
 * protocol `messages`, `tools` with `inputSchema`) and provider wire shapes.
 */

export interface PromptPart {
  role: string;
  label: string;
  content: string;
  tokens: number;
}

export interface PromptTool {
  name: string;
  description: string;
  schema: string;
  tokens: number;
}

export interface PromptInspect {
  model: string;
  sent: boolean;
  messages: PromptPart[];
  tools: PromptTool[];
  totalTokens: number;
}

type Obj = Record<string, unknown>;

const obj = (v: unknown): Obj | null => (v && typeof v === "object" && !Array.isArray(v) ? (v as Obj) : null);
const arr = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
const s = (v: unknown): string => (typeof v === "string" ? v : "");

/** Rough token estimate (4 chars per token), matching the engine's fallback. */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

function json(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

function blockText(block: unknown): string {
  if (typeof block === "string") return block;
  const b = obj(block);
  if (!b) return "";
  const type = s(b.type);
  if (type === "text") return s(b.text);
  if (type === "thinking") return `[thinking]\n${s(b.text) || s(b.thinking)}`;
  if (type === "redactedThinking" || type === "redacted_thinking") return "[redacted thinking]";
  if (type === "image" || type === "document") return `[${type}]`;
  if (type === "toolUse" || type === "tool_use") return `→ ${s(b.name)} ${json(b.input)}`;
  if (type === "toolResult" || type === "tool_result") {
    const content = Array.isArray(b.content) ? b.content.map(blockText).join("\n") : s(b.content);
    return `← ${s(b.toolUseId) || s(b.tool_use_id)}${b.isError || b.is_error ? " (error)" : ""}\n${content}`;
  }
  return s(b.text) || json(b);
}

function messageLabel(role: string, content: unknown): string {
  const blocks = arr(content).map(obj);
  const types = new Set(blocks.map((b) => s(b?.type)));
  if (role === "tool" || (types.size > 0 && [...types].every((t) => t === "toolResult" || t === "tool_result"))) {
    return "Tool results";
  }
  if (role === "assistant") return types.has("toolUse") || types.has("tool_use") ? "Assistant · tool calls" : "Assistant";
  if (role === "system") return "System";
  return "User";
}

function part(role: string, label: string, content: string): PromptPart {
  return { role, label, content, tokens: estimateTokens(content) };
}

function systemParts(system: unknown): PromptPart[] {
  const blocks = typeof system === "string" ? [system] : arr(system);
  return blocks.flatMap((block, i) => {
    const text = typeof block === "string" ? block : s(obj(block)?.text);
    if (!text.trim()) return [];
    const heading = /^#+\s+(.+)$/m.exec(text.trim().split("\n")[0] ?? "")?.[1]?.trim();
    const label = i === 0 ? "System" : heading ? heading.slice(0, 48) : `System ${i + 1}`;
    return [part("system", label, text)];
  });
}

function messageParts(messages: unknown): PromptPart[] {
  return arr(messages).flatMap((raw) => {
    const m = obj(raw);
    if (!m) return [];
    const role = s(m.role) || "user";
    const content = m.content;
    const body = typeof content === "string" ? content : arr(content).map(blockText).filter(Boolean).join("\n\n");
    const calls = arr(m.tool_calls)
      .map((c) => obj(obj(c)?.function))
      .filter((f): f is Obj => f !== null)
      .map((f) => `→ ${s(f.name)} ${s(f.arguments)}`);
    const text = [body, ...calls].filter(Boolean).join("\n\n");
    return text ? [part(role, messageLabel(role, content), text)] : [];
  });
}

function toolParts(tools: unknown): PromptTool[] {
  return arr(tools).flatMap((raw) => {
    const t = obj(raw);
    const fn = obj(t?.function) ?? t;
    if (!fn || !s(fn.name)) return [];
    const schema = json(fn.inputSchema ?? fn.input_schema ?? fn.parameters ?? {});
    const description = s(fn.description);
    return [{ name: s(fn.name), description, schema, tokens: estimateTokens(description + schema) }];
  });
}

export function parseInspectRequest(raw: unknown): PromptInspect | null {
  const request = obj(raw);
  if (!request) return null;
  const messages = [...systemParts(request.system), ...messageParts(request.messages)];
  const tools = toolParts(request.tools);
  const totalTokens =
    messages.reduce((n, m) => n + m.tokens, 0) + tools.reduce((n, t) => n + t.tokens, 0);
  return { model: s(request.model), sent: true, messages, tools, totalTokens };
}
