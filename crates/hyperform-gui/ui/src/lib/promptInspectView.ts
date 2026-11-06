import type { PromptInspect, PromptPart, PromptTool } from "./domain/requestInspect";
import { promptInsights } from "./promptInsights";

export type InspectRow =
  | { key: string; kind: "msg"; part: PromptPart }
  | { key: string; kind: "tool"; tool: PromptTool };

export type ContextCategory = "instructions" | "project" | "conversation" | "capabilities";

export function categorizeRow(row: InspectRow): ContextCategory {
  if (row.kind === "tool") return "capabilities";
  const label = row.part.label.toLowerCase();
  const role = row.part.role.toLowerCase();
  if (
    label.includes("repo") ||
    label.includes("agents") ||
    label.includes("map") ||
    label.includes("context") ||
    label.includes("note")
  ) {
    return "project";
  }
  if (role === "system") return "instructions";
  return "conversation";
}

/** Plain names and the context-map color for each kind of prompt part. */
export function categoryMeta(cat: ContextCategory): { label: string; desc: string; color: string } {
  switch (cat) {
    case "instructions":
      return {
        label: "Instructions",
        desc: "The system prompt: how the agent works and the rules it follows.",
        color: "var(--layer-system)",
      };
    case "project":
      return {
        label: "Project",
        desc: "What the agent knows about this project: AGENTS.md, the repository map and saved notes.",
        color: "var(--layer-instructions)",
      };
    case "conversation":
      return {
        label: "Conversation",
        desc: "This chat's messages and tool results so far.",
        color: "var(--layer-messages)",
      };
    case "capabilities":
      return {
        label: "Tools",
        desc: "The tools the agent may call, with the input each one accepts.",
        color: "var(--layer-tools)",
      };
  }
}

export function inspectRows(snap: PromptInspect): InspectRow[] {
  return [
    ...snap.messages.map((part, i) => ({ key: `m-${i}`, kind: "msg" as const, part })),
    ...snap.tools.map((tool, i) => ({ key: `t-${i}`, kind: "tool" as const, tool })),
  ];
}

export function inspectBody(row: InspectRow): string {
  if (row.kind === "msg") return row.part.content;
  return `${row.tool.description}\n\n${row.tool.schema}`;
}

export function inspectCopyText(snap: PromptInspect): string {
  const ins = promptInsights(snap);
  const chunks = [
    `model: ${snap.model}`,
    snap.sent ? "sent: yes" : "sent: preview / reconstructed",
    `tokens ≈ ${snap.totalTokens}`,
    `largest: ${ins.largest.name} (${Math.round(ins.largest.share * 100)}%)`,
    `stable prefix: ${ins.stablePrefix}`,
    `volatile tail: ${ins.volatileTail}`,
    "",
    "## Order (wire)",
    ...ins.layers.map(
      (l) => `${l.order}. ${l.label} [${l.role}] ~${l.tokens} tok (${Math.round(l.share * 100)}%)`,
    ),
    "",
    "## Hints",
    ...ins.hints.map((h) => `- ${h}`),
    "",
  ];
  for (const m of snap.messages) {
    chunks.push(`## ${m.label} (${m.role}, ~${m.tokens} tok)`, m.content, "");
  }
  if (snap.tools.length) {
    chunks.push("## Tools");
    for (const t of snap.tools) {
      chunks.push(`### ${t.name} (~${t.tokens} tok)`, t.description, t.schema, "");
    }
  }
  return chunks.join("\n");
}
