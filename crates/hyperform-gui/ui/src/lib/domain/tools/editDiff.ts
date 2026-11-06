import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import { list, str } from "./toolInput";

export interface DiffOp {
  kind: "ctx" | "del" | "add";
  text: string;
}

/** Above this many LCS cells the diff degrades to delete-all/add-all. */
const MAX_CELLS = 250_000;
const MAX_WRITE_LINES = 2000;

function lines(text: string): string[] {
  if (!text) return [];
  const parts = text.split("\n");
  if (parts[parts.length - 1] === "") parts.pop();
  return parts;
}

const op = (kind: DiffOp["kind"]) => (text: string): DiffOp => ({ kind, text });

/** Line diff (LCS) after trimming the common prefix and suffix. */
export function lineDiff(a: string[], b: string[]): DiffOp[] {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  const midA = a.slice(start, endA);
  const midB = b.slice(start, endB);
  const head = a.slice(0, start).map(op("ctx"));
  const tail = a.slice(endA).map(op("ctx"));
  if (midA.length * midB.length > MAX_CELLS) {
    return [...head, ...midA.map(op("del")), ...midB.map(op("add")), ...tail];
  }
  const rows = midA.length;
  const cols = midB.length;
  const dp = Array.from({ length: rows + 1 }, () => new Uint32Array(cols + 1));
  for (let i = rows - 1; i >= 0; i--) {
    for (let j = cols - 1; j >= 0; j--) {
      dp[i][j] = midA[i] === midB[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const mid: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < rows && j < cols) {
    if (midA[i] === midB[j]) {
      mid.push({ kind: "ctx", text: midA[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      mid.push({ kind: "del", text: midA[i++] });
    } else {
      mid.push({ kind: "add", text: midB[j++] });
    }
  }
  while (i < rows) mid.push({ kind: "del", text: midA[i++] });
  while (j < cols) mid.push({ kind: "add", text: midB[j++] });
  return [...head, ...mid, ...tail];
}

const SIGN = { ctx: " ", del: "-", add: "+" } as const;

/** Unified hunks with `context` lines around each change; line numbers are snippet-relative. */
export function unifiedHunks(ops: DiffOp[], context = 3): string[] {
  const changed = ops.flatMap((o, i) => (o.kind === "ctx" ? [] : [i]));
  if (changed.length === 0) return [];
  const ranges: Array<[number, number]> = [];
  for (const index of changed) {
    const from = Math.max(0, index - context);
    const to = Math.min(ops.length, index + context + 1);
    const last = ranges[ranges.length - 1];
    if (last && from <= last[1]) last[1] = Math.max(last[1], to);
    else ranges.push([from, to]);
  }
  const oldAt: number[] = [];
  const newAt: number[] = [];
  let oldLine = 1;
  let newLine = 1;
  for (const o of ops) {
    oldAt.push(oldLine);
    newAt.push(newLine);
    if (o.kind !== "add") oldLine++;
    if (o.kind !== "del") newLine++;
  }
  const out: string[] = [];
  for (const [from, to] of ranges) {
    const slice = ops.slice(from, to);
    const oldCount = slice.filter((o) => o.kind !== "add").length;
    const newCount = slice.filter((o) => o.kind !== "del").length;
    out.push(`@@ -${oldAt[from]},${oldCount} +${newAt[from]},${newCount} @@`);
    for (const o of slice) out.push(`${SIGN[o.kind]}${o.text}`);
  }
  return out;
}

function header(path: string, created: boolean): string[] {
  return [created ? "--- /dev/null" : `--- a/${path}`, `+++ b/${path}`];
}

function createdFile(path: string, content: string): string {
  const body = lines(content).slice(0, MAX_WRITE_LINES);
  return [...header(path, true), `@@ -0,0 +1,${body.length} @@`, ...body.map((l) => `+${l}`)].join("\n");
}

/**
 * A unified diff built from Edit / MultiEdit / Write / NotebookEdit input, or
 * null. `displayPath` (e.g. project-relative) replaces the path in the headers.
 */
export function editDiff(name: string, input: JsonValue, displayPath?: string): string | null {
  const path = displayPath || str(input, "file_path", "notebook_path", "path");
  if (name === "Write") return path ? createdFile(path, str(input, "content")) : null;
  if (name === "NotebookEdit") return path ? createdFile(path, str(input, "new_source")) : null;
  const edits =
    name === "MultiEdit"
      ? list(input, "edits").map((e) => ({ before: str(e, "old_string"), after: str(e, "new_string") }))
      : [{ before: str(input, "old_string"), after: str(input, "new_string") }];
  const hunks = edits.flatMap((e) => unifiedHunks(lineDiff(lines(e.before), lines(e.after))));
  if (!path || hunks.length === 0) return null;
  return [...header(path, false), ...hunks].join("\n");
}
