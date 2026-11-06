import type { PromptInspect, PromptPart, PromptTool } from "./domain/requestInspect";

export interface PromptLayer {
  order: number;
  kind: "message" | "tool";
  label: string;
  role: string;
  tokens: number;
  share: number;
  chars: number;
  lines: number;
  cacheable: boolean;
}

export interface PromptStackSlice {
  id: string;
  label: string;
  tokens: number;
  share: number;
  color: string;
}

export interface PromptInsights {
  layers: PromptLayer[];
  stack: PromptStackSlice[];
  largest: { name: string; tokens: number; share: number };
  stablePrefix: string;
  volatileTail: string;
  cacheableTokens: number;
  volatileTokens: number;
  hints: string[];
}

const STACK_COLORS: Record<string, string> = {
  system: "#94a3b8",
  user: "#f0a090",
  assistant: "#7dd3fc",
  tool: "#a78bfa",
  "tool def": "#818cf8",
};

function layer(
  order: number,
  kind: PromptLayer["kind"],
  label: string,
  role: string,
  tokens: number,
  total: number,
  content: string,
  cacheable: boolean,
): PromptLayer {
  const chars = content.length;
  const lines = content ? content.split("\n").length : 0;
  return {
    order,
    kind,
    label,
    role,
    tokens,
    share: total > 0 ? tokens / total : 0,
    chars,
    lines,
    cacheable,
  };
}

function isCacheable(kind: PromptLayer["kind"], label: string, role: string): boolean {
  if (kind === "tool") return true;
  return role === "system" && label === "System";
}

/** Case-study view of a prompt: send order, budget share, optimization hints. */
export function promptInsights(snap: PromptInspect): PromptInsights {
  const total = Math.max(1, snap.totalTokens);
  const layers: PromptLayer[] = [
    ...snap.messages.map((m: PromptPart, i) =>
      layer(
        i + 1,
        "message",
        m.label,
        m.role,
        m.tokens,
        total,
        m.content,
        isCacheable("message", m.label, m.role),
      ),
    ),
    ...snap.tools.map((t: PromptTool, i) =>
      layer(
        snap.messages.length + i + 1,
        "tool",
        t.name,
        "tool def",
        t.tokens,
        total,
        `${t.description}\n${t.schema}`,
        true,
      ),
    ),
  ];
  const ranked = [...layers].sort((a, b) => b.tokens - a.tokens);
  const biggest = ranked[0] ?? layer(1, "message", "empty", "none", 0, 1, "", true);
  const msgLabels = snap.messages.map((m) => m.label);
  const hints: string[] = [];
  const toolTok = snap.tools.reduce((n, t) => n + t.tokens, 0);
  if (!snap.sent) {
    hints.push(
      "This snapshot has not been sent this process — restore or send a turn to capture the live request.",
    );
  }
  if (toolTok / total > 0.35) {
    hints.push(
      `Tool schemas are ${Math.round((toolTok / total) * 100)}% of the request. Drop unused tools or shorten descriptions to cut this.`,
    );
  }
  const repo = snap.messages.find((m) => m.label === "Repo map");
  if (repo && repo.tokens / total > 0.15) {
    hints.push("Repo map is a large volatile block. Narrow tracked files or lower the map budget.");
  }
  const toolResults = snap.messages.filter((m) => m.role === "tool");
  if (toolResults.length >= 4) {
    hints.push(
      `${toolResults.length} tool results are still in the working set. Mark droppable output or compact to shrink the tail.`,
    );
  }
  const sys = snap.messages.find((m) => m.label === "System");
  if (sys) {
    hints.push(
      "Keep the system prefix byte-stable (no clocks or counters) so the provider cache can reuse it across turns.",
    );
  }
  if (ranked[0] && ranked[0].share > 0.4) {
    hints.push(
      `Largest sink: ${ranked[0].label} (~${Math.round(ranked[0].share * 100)}%). Start optimization there.`,
    );
  }
  const cacheableTokens = layers.filter((l) => l.cacheable).reduce((n, l) => n + l.tokens, 0);
  const volatileTokens = Math.max(0, snap.totalTokens - cacheableTokens);
  const byRole = new Map<string, number>();
  for (const l of layers) {
    byRole.set(l.role, (byRole.get(l.role) ?? 0) + l.tokens);
  }
  const stack: PromptStackSlice[] = [...byRole.entries()].map(([id, tokens]) => ({
    id,
    label: id === "tool def" ? "Tool schemas" : id[0].toUpperCase() + id.slice(1),
    tokens,
    share: tokens / total,
    color: STACK_COLORS[id] ?? "#8b8b96",
  }));
  return {
    layers,
    stack,
    largest: { name: biggest.label, tokens: biggest.tokens, share: biggest.share },
    stablePrefix: msgLabels[0] ?? "System",
    volatileTail: msgLabels.filter((l) => l !== "System").slice(-2).join(" → ") || "working set",
    cacheableTokens,
    volatileTokens,
    hints,
  };
}
