import type { SettingsTab } from "../../stores/ui.svelte";

/**
 * What Settings can find: every page grouped as the navigation shows it,
 * and the settings on each page with the words people use for them. A
 * result opens its page and scrolls to the row whose `data-setting` is
 * the entry's key.
 */

export interface SettingsSection {
  label: string;
  tabs: SettingsTab[];
}

export const SETTINGS_SECTIONS: readonly SettingsSection[] = [
  { label: "General", tabs: ["models", "providers", "appearance", "pet"] },
  { label: "Agent", tabs: ["permissions", "memory", "verification", "extensions"] },
  { label: "Integrations", tabs: ["mcp", "hooks"] },
  { label: "System", tabs: ["advanced", "experimental", "about"] },
];

export interface SettingEntry {
  tab: SettingsTab;
  /** A key path joined with dots, or `@Title` for a group or list. */
  key: string;
  title: string;
  /** The settings group (by title) that holds it, when that group can fold. */
  group: string | null;
  words: string;
}

const e = (tab: SettingsTab, key: string, title: string, group: string | null, words = ""): SettingEntry => ({
  tab,
  key,
  title,
  group,
  words,
});

export const SETTING_ENTRIES: readonly SettingEntry[] = [
  e("models", "model.main", "Main model", "Models", "default llm claude gpt which model answers"),
  e("models", "model.fast", "Fast model", "Models", "cheap small quick haiku summaries compaction"),
  e("models", "model.review", "Review model", "Models", "reviewer second opinion"),
  e("models", "model.fallbacks", "Fallback models", "Fallbacks", "backup retry outage"),
  e("models", "model.effort", "Reasoning effort", "Requests", "thinking budget think harder"),
  e("models", "model.max_output_tokens", "Max output tokens", "Requests", "reply length limit"),
  e("models", "model.context_window", "Context window", "Requests", "tokens size limit"),
  e("providers", "@Providers", "Providers and API keys", null, "api key credentials anthropic openai openrouter zen base url login connect"),
  e("pet", "ui.pet.name", "Pet name", null, "companion mascot call rename"),
  e("pet", "ui.pet.look", "Pet look", null, "companion color skin pearl mint sky lilac peach graphite"),
  e("pet", "ui.companion", "Pet liveliness", null, "companion mascot island orb lively calm off animation"),
  e("pet", "ui.pet.roam", "Let the pet roam", null, "companion walk wander composer panels stay put"),
  e("appearance", "ui.task_report_view", "Task report detail", null, "receipt summary verification turn end density"),
  e("appearance", "ui.output_style", "Output style", null, "response style tone answers explanatory"),
  e("permissions", "permissions.mode", "Permission mode", null, "approvals ask bypass accept edits yolo"),
  e("permissions", "permissions.planning", "Start new chats in plan mode", null, "plan mode first research propose read-only"),
  e("permissions", "permissions.auto_allow_read_only_bash", "Run read-only shell commands without asking", null, "bash ls cat git status approvals"),
  e("permissions", "@Allow", "Allow rules", null, "permission rule allowlist always allow"),
  e("permissions", "@Ask", "Ask rules", null, "permission rule confirm"),
  e("permissions", "@Deny", "Deny rules", null, "permission rule block forbid never"),
  e("permissions", "permissions.additional_directories", "Additional directories", "Folders", "folders outside project access paths"),
  e("memory", "@Instruction files", "Instruction files", null, "agents.md claude.md memory instructions rules notes"),
  e("verification", "verification.mode", "Verification mode", null, "checks tests after changes verify"),
  e("verification", "verification.max_continuations", "Max continuations", null, "retries keep working failing checks"),
  e("verification", "verification.auto_checks", "Auto checks", null, "detect tests lint build"),
  e("verification", "@Checks", "Checks", null, "test lint build typecheck commands"),
  e("extensions", "@Agents", "Agents, commands and skills", null, "subagents slash commands skills rules output styles custom"),
  e("mcp", "@MCP", "MCP servers", null, "model context protocol tools servers stdio http integrations"),
  e("hooks", "@Events", "Hooks", null, "hook script command event pretooluse posttooluse sessionstart stop precompact automation"),
  e("advanced", "context.compact_at_percent", "Compact at", "Context", "compaction summarize history context full"),
  e("advanced", "context.keep_recent_tool_results", "Recent tool results kept", "Context", "clear old tool output"),
  e("advanced", "context.repo_map", "Repository map", "Context", "repo outline files symbols"),
  e("advanced", "context.repo_map_chars", "Repository map size", "Context", "repo outline characters"),
  e("advanced", "agents.max_concurrent", "Subagents at once", "Agent limits", "parallel helpers concurrency"),
  e("advanced", "agents.max_depth", "Subagent depth", "Agent limits", "nesting helpers"),
  e("advanced", "agents.max_turns", "Turns per agent run", "Agent limits", "steps limit loop"),
  e("advanced", "agents.session_cost_cap_usd", "Session cost cap", "Agent limits", "budget money spend dollars limit price"),
  e("advanced", "web.search_backend", "Search backend", "Web", "web search searxng brave duckduckgo internet"),
  e("advanced", "web.search_url", "SearXNG URL", "Web", "web search server"),
  e("advanced", "web.fetch_extract", "Answer from fetched pages with the fast model", "Web", "webfetch summarize pages"),
  e("advanced", "web.allow_private_network", "Allow private network", "Web", "localhost intranet fetch"),
  e("advanced", "shell.path", "Shell", "Shell", "bash zsh powershell terminal"),
  e("advanced", "shell.env", "Environment variables", "Shell", "env vars path"),
  e("advanced", "shell.sandbox.enabled", "Sandbox commands", "Shell", "isolation seatbelt safety"),
  e("advanced", "shell.sandbox.auto_allow", "Run sandboxed commands without asking", "Shell", "approvals sandbox"),
  e("advanced", "shell.sandbox.allow_network", "Allow network in the sandbox", "Shell", "internet sandbox"),
  e("advanced", "lsp.enabled", "Use language servers", "Language servers", "lsp diagnostics rust-analyzer typescript"),
  e("advanced", "compat.claude", "Read .claude folders", "Workspace", "claude code compatibility agents commands"),
  e("experimental", "@Experimental features", "Experimental features", null, "beta preview flags shadow try new decision model laya native onnx download runtime"),
  e("about", "@Updates", "Updates", null, "version upgrade release install about"),
];

function rank(entry: SettingEntry, words: string[]): number | null {
  const title = entry.title.toLowerCase();
  const hay = `${title} ${entry.words} ${entry.key.toLowerCase()}`;
  if (!words.every((w) => hay.includes(w))) return null;
  const phrase = words.join(" ");
  if (title.startsWith(phrase)) return 0;
  if (title.includes(phrase)) return 1;
  return words.every((w) => title.includes(w)) ? 2 : 3;
}

/** The settings matching every word of `query`, best first; `extra` adds entries known only at runtime. */
export function searchSettings(query: string, limit = 8, extra: readonly SettingEntry[] = []): SettingEntry[] {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return [...SETTING_ENTRIES, ...extra].map((entry, order) => ({ entry, order, score: rank(entry, words) }))
    .filter((r): r is { entry: SettingEntry; order: number; score: number } => r.score !== null)
    .sort((a, b) => a.score - b.score || a.order - b.order)
    .slice(0, limit)
    .map((r) => r.entry);
}

/** The foldable group holding a setting. */
export function groupOfSetting(key: string | null): string | null {
  return SETTING_ENTRIES.find((entry) => entry.key === key)?.group ?? null;
}
