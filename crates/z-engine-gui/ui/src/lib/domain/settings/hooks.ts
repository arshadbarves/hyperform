import type { HookConfig } from "../../protocol/config/HookConfig";
import { isRecord, rawAt } from "./provenance";

/** What an event's matcher (a regex) is tested against, for the form. */
export interface HookMatcherMeta {
  label: string;
  placeholder: string;
  /** Shown in the list when a hook has no matcher. */
  any: string;
}

export interface HookEventMeta {
  name: string;
  description: string;
  /** Null for events that run every hook; the engine ignores matchers there. */
  matcher: HookMatcherMeta | null;
}

const TOOL_MATCHER: HookMatcherMeta = { label: "Tools", placeholder: "Bash|Edit (blank matches every tool)", any: "every tool" };

/** Hook events, named as in Claude Code; the config crate rejects any other name. */
export const HOOK_EVENTS: readonly HookEventMeta[] = [
  {
    name: "SessionStart",
    description: "When a session starts or resumes. Stdout is added as context.",
    matcher: { label: "Start source", placeholder: "startup or resume (blank matches both)", any: "startup and resume" },
  },
  {
    name: "UserPromptSubmit",
    description: "Before a prompt reaches the model. Stdout is added as context; exit 2 blocks the prompt.",
    matcher: null,
  },
  {
    name: "PreToolUse",
    description: "Before a tool runs. Can block the call, rewrite its input, or decide the approval.",
    matcher: TOOL_MATCHER,
  },
  { name: "PostToolUse", description: "After a tool finishes. Its output is appended to the tool result.", matcher: TOOL_MATCHER },
  {
    name: "Stop",
    description: "When the main agent ends its response. Exit 2 keeps it working with stderr as the reason.",
    matcher: null,
  },
  { name: "SubagentStop", description: "When a subagent ends its response; exit 2 keeps it working.", matcher: null },
  {
    name: "PreCompact",
    description: "Before older history is summarized to free context.",
    matcher: { label: "Trigger", placeholder: "auto or manual (blank matches both)", any: "auto and manual" },
  },
  { name: "Notification", description: "When the app notifies you, for example about a waiting approval.", matcher: null },
  { name: "SessionEnd", description: "When a session closes.", matcher: null },
];

export const DEFAULT_HOOK_TIMEOUT = 60;

export function emptyHook(): HookConfig {
  return { matcher: null, command: "", timeout_secs: DEFAULT_HOOK_TIMEOUT };
}

/** A layer file's hook entry with the defaults the loader applies. */
export function normalizeHook(raw: unknown): HookConfig {
  const entry = isRecord(raw) ? raw : {};
  const matcher = typeof entry.matcher === "string" && entry.matcher.trim() ? entry.matcher : null;
  const timeout = typeof entry.timeout_secs === "number" ? entry.timeout_secs : DEFAULT_HOOK_TIMEOUT;
  return { matcher, command: typeof entry.command === "string" ? entry.command : "", timeout_secs: timeout };
}

/** The hooks one layer file defines for `event`, in run order. */
export function layerHooks(raw: unknown, event: string): HookConfig[] {
  const list = rawAt(raw, ["hooks", event]);
  return Array.isArray(list) ? list.map(normalizeHook) : [];
}

/** Why the hook cannot be saved, or null. Matchers use Rust regex syntax,
 * which has no lookaround or backreferences. */
export function hookError(hook: HookConfig, event: HookEventMeta): string | null {
  if (!hook.command.trim()) return "Enter the command to run.";
  if (!Number.isInteger(hook.timeout_secs) || hook.timeout_secs < 1) return "The timeout must be at least 1 second.";
  const matcher = hook.matcher?.trim();
  if (!event.matcher || !matcher) return null;
  if (/\(\?<?[=!]|\\[1-9]/.test(matcher)) return "Matchers do not support lookaround or backreferences.";
  try {
    new RegExp(matcher);
  } catch {
    return "The matcher is not a valid regular expression.";
  }
  return null;
}

/** The hook as written to the file: no matcher for events that ignore one. */
export function hookForEvent(hook: HookConfig, event: HookEventMeta): HookConfig {
  const matcher = event.matcher ? hook.matcher?.trim() || null : null;
  return { matcher, command: hook.command.trim(), timeout_secs: hook.timeout_secs };
}
