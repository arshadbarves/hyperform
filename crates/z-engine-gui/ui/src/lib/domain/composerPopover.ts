import { activeAtToken } from "../atFile";
import type { AgentCard } from "../commands/engine";
import { isRememberText } from "./composerSubmit";
import { slashQuery } from "./slashCommands";

export type ComposerPopover =
  | { kind: "slash"; query: string }
  | { kind: "mention"; query: string }
  | { kind: "remember" };

export type MentionItem = { kind: "agent"; agent: AgentCard } | { kind: "file"; path: string };

/** Which suggestion list the caret is in: `/command`, `#memory`, or an `@mention`. */
export function activePopover(text: string, caret: number): ComposerPopover | null {
  const slash = slashQuery(text);
  if (slash !== null && caret <= text.length) return { kind: "slash", query: slash };
  if (isRememberText(text) && !text.slice(0, caret).includes("\n")) return { kind: "remember" };
  const mention = activeAtToken(text, caret);
  return mention === null ? null : { kind: "mention", query: mention };
}

export function filterAgents(agents: AgentCard[], query: string, max = 6): AgentCard[] {
  const q = query.replace(/^agent-/, "").toLowerCase();
  return agents
    .filter((a) => !q || a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q))
    .slice(0, max);
}

/** Text inserted for an agent mention (`@agent-reviewer `). */
export function agentMention(agent: AgentCard): string {
  return `@agent-${agent.name} `;
}

export function wrapIndex(index: number, delta: number, length: number): number {
  if (length <= 0) return 0;
  return (index + delta + length) % length;
}
