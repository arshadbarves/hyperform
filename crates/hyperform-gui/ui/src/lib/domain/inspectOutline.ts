import { categorizeRow, categoryMeta, type ContextCategory, type InspectRow } from "../promptInspectView";

/** The prompt inspector's outline: the request's parts by what each is for. */

export const CATEGORY_ORDER: ContextCategory[] = ["instructions", "project", "conversation", "capabilities"];

export interface OutlineItem {
  /** Index into the inspect rows. */
  index: number;
  label: string;
  role: string;
  tokens: number;
  /** A conversation part's first line, so repeated "User" rows can be told apart. */
  detail: string | null;
}

const DETAIL_MAX = 72;

/** The first line that says something; a tool result's `← id` header does not. */
function firstLine(text: string): string | null {
  const line = text.split("\n").find((l) => l.trim() && !l.startsWith("← "))?.trim() ?? "";
  if (!line) return null;
  return line.length > DETAIL_MAX ? `${line.slice(0, DETAIL_MAX - 1)}…` : line;
}

export interface OutlineGroup {
  category: ContextCategory;
  label: string;
  tokens: number;
  items: OutlineItem[];
}

function itemOf(row: InspectRow, index: number, category: ContextCategory): OutlineItem {
  if (row.kind === "tool") return { index, label: row.tool.name, role: "tool definition", tokens: row.tool.tokens, detail: null };
  const detail = category === "conversation" ? firstLine(row.part.content) : null;
  return { index, label: row.part.label, role: row.part.role, tokens: row.part.tokens, detail };
}

export function outlineGroups(rows: InspectRow[], query: string, only: ContextCategory | null): OutlineGroup[] {
  const q = query.trim().toLowerCase();
  const groups = new Map<ContextCategory, OutlineGroup>();
  rows.forEach((row, index) => {
    const category = categorizeRow(row);
    if (only && category !== only) return;
    const item = itemOf(row, index, category);
    const body = row.kind === "msg" ? row.part.content : `${row.tool.description}\n${row.tool.schema}`;
    if (q && !`${item.label}\n${item.role}\n${body}`.toLowerCase().includes(q)) return;
    const group = groups.get(category) ?? { category, label: categoryMeta(category).label, tokens: 0, items: [] };
    group.items.push(item);
    group.tokens += item.tokens;
    groups.set(category, group);
  });
  return CATEGORY_ORDER.flatMap((c) => groups.get(c) ?? []);
}

/** The next visible row for ↑ / ↓; the first one when the current is hidden. */
export function stepSelection(visible: number[], current: number, dir: -1 | 1): number | null {
  if (visible.length === 0) return null;
  const at = visible.indexOf(current);
  if (at === -1) return visible[0] ?? null;
  return visible[Math.max(0, Math.min(visible.length - 1, at + dir))] ?? null;
}

/** Markdown for the Reader view of one part. */
export function readerSource(row: InspectRow): string {
  if (row.kind === "tool") return `${row.tool.description}\n\n\`\`\`json\n${row.tool.schema}\n\`\`\``;
  if (row.part.role === "tool") return `\`\`\`\n${row.part.content}\n\`\`\``;
  return row.part.content;
}
