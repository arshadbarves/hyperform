import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import { fmtDuration } from "../format";
import type { ToolCallStatus } from "../sessionView/types";
import { activityLabel } from "../tools/activityLabel";
import { str } from "../tools/toolInput";
import { toolMeta, type ToolFamily } from "../tools/toolMeta";
import type { ToolUseRef } from "./blocks";
import type { TimelineItem } from "./turns";

/** A transcript block: one item, or a run of consecutive tool calls shown as one line. */
export type TurnBlock =
  | { kind: "item"; key: string; item: TimelineItem }
  | { kind: "run"; key: string; uses: ToolUseRef[] };

/** Calls that matter on their own: a subagent, a question, a plan. */
const SOLO: ReadonlySet<ToolFamily> = new Set(["agent", "question", "plan"]);

function foldable(item: TimelineItem): item is Extract<TimelineItem, { kind: "tool" }> {
  return item.kind === "tool" && !SOLO.has(toolMeta(item.use.name).family);
}

/** Folds two or more consecutive tool calls into a run; a lone call keeps its own card. */
export function groupTurnItems(items: TimelineItem[]): TurnBlock[] {
  const blocks: TurnBlock[] = [];
  let pending: Extract<TimelineItem, { kind: "tool" }>[] = [];
  const flush = () => {
    if (pending.length >= 2) {
      blocks.push({ kind: "run", key: `run:${pending[0].use.callId}`, uses: pending.map((p) => p.use) });
    } else {
      for (const item of pending) blocks.push({ kind: "item", key: item.key, item });
    }
    pending = [];
  };
  for (const item of items) {
    if (foldable(item)) {
      pending.push(item);
      continue;
    }
    flush();
    blocks.push({ kind: "item", key: item.key, item });
  }
  flush();
  return blocks;
}

export interface RunCall {
  name: string;
  input: JsonValue;
  status: ToolCallStatus;
}

export interface RunSummary {
  /** Plain phrases in the order the work happened: "read 2 files", "ran 1 command". */
  parts: string[];
  state: "running" | "ok" | "failed";
  /** The step in progress, in words, while the run is live. */
  current: string | null;
  failed: number;
  denied: number;
}

function n(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

function pathOf(input: JsonValue): string {
  return str(input, "file_path", "notebook_path", "path");
}

/** What a run of tool calls did, counted by kind, with its live step and failures. */
export function summarizeRun(calls: RunCall[]): RunSummary {
  const order: string[] = [];
  const files = new Map<string, Set<string>>();
  const counts = new Map<string, number>();
  const bump = (key: string, path?: string) => {
    if (!counts.has(key)) order.push(key);
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (path !== undefined) {
      const set = files.get(key) ?? new Set<string>();
      set.add(path);
      files.set(key, set);
    }
  };
  for (const c of calls) {
    const meta = toolMeta(c.name);
    if (meta.family === "read" || meta.family === "edit" || meta.family === "write") bump(meta.family, pathOf(c.input));
    else if (meta.family === "mcp") bump(`mcp:${meta.server ?? ""}`);
    else if (meta.family === "web") bump(c.name === "WebSearch" ? "websearch" : "webfetch");
    else if (["search", "bash", "todo", "job"].includes(meta.family)) bump(meta.family);
    else bump("other");
  }
  const parts = order.map((key) => {
    const count = counts.get(key) ?? 0;
    const distinct = files.get(key)?.size ?? count;
    if (key === "read") return `read ${n(distinct, "file")}`;
    if (key === "edit") return `edited ${n(distinct, "file")}`;
    if (key === "write") return `wrote ${n(distinct, "file")}`;
    if (key === "search") return `searched ${count}×`;
    if (key === "bash") return `ran ${n(count, "command")}`;
    if (key === "webfetch") return `read ${n(count, "web page")}`;
    if (key === "websearch") return count === 1 ? "searched the web" : `searched the web ${count}×`;
    if (key === "todo") return "updated the plan";
    if (key === "job") return `checked ${n(count, "background job")}`;
    if (key.startsWith("mcp:")) return `used ${key.slice(4)} ${count}×`;
    return `used ${n(count, "other tool")}`;
  });
  const live = [...calls].reverse().find((c) => c.status === "running");
  const failed = calls.filter((c) => c.status === "error").length;
  const denied = calls.filter((c) => c.status === "denied").length;
  return {
    parts,
    state: live ? "running" : failed || denied ? "failed" : "ok",
    current: live ? activityLabel(live.name, live.input) : null,
    failed,
    denied,
  };
}

/** The run's one line, capitalized: "Read 2 files · ran 1 command". */
export function runLine(summary: RunSummary): string {
  const line = summary.parts.join(" · ");
  return line.charAt(0).toUpperCase() + line.slice(1);
}

/** A turn split for display: the work that led to the answer, and the answer. */
export interface WorkSection {
  /** Thinking, tool calls and the narration between them, in order. */
  work: TimelineItem[];
  /** Everything after the last thinking or tool call: the answer and any cards after it. */
  answer: TimelineItem[];
  /** Work that stays in view under the folded line: agents, questions, plans, failures, steering, errors, output, compaction. */
  pinned: TimelineItem[];
  /** The calls the folded line counts (agents, questions and plans stand on their own). */
  uses: ToolUseRef[];
  /** Fold the work into one line: the turn is done and made two or more tool calls. */
  fold: boolean;
}

function staysInView(item: TimelineItem, failed: (callId: string) => boolean): boolean {
  if (item.kind === "tool") return !foldable(item) || failed(item.use.callId);
  return item.kind !== "thinking" && item.kind !== "text";
}

/**
 * Gathers a turn's thinking and tool calls (with the narration between them)
 * into one Work block ahead of its answer. `failed` names the calls that
 * failed; they, and anything that stands on its own, stay visible when the
 * block folds.
 */
export function workSection(items: TimelineItem[], done: boolean, failed: (callId: string) => boolean = () => false): WorkSection {
  const last = items.findLastIndex((item) => item.kind === "thinking" || item.kind === "tool");
  const work = items.slice(0, last + 1);
  const calls = work.filter((item): item is Extract<TimelineItem, { kind: "tool" }> => item.kind === "tool");
  return {
    work,
    answer: items.slice(last + 1),
    pinned: work.filter((item) => staysInView(item, failed)),
    uses: calls.filter(foldable).map((item) => item.use),
    fold: done && calls.length >= 2,
  };
}

const TRAILING: ReadonlySet<TimelineItem["kind"]> = new Set(["compaction", "taskView", "trace"]);

/** A turn's items and the dividers and plan traces after its last item, which belong after the turn's actions. */
export function splitTrailingCompactions(items: TimelineItem[]): { body: TimelineItem[]; after: TimelineItem[] } {
  let end = items.length;
  while (end > 0 && TRAILING.has(items[end - 1].kind)) end--;
  return { body: items.slice(0, end), after: items.slice(end) };
}

/** The folded Work block in one line: "Worked for 1m 12s · read 6 files · ran 2 commands · 2 tasks done". */
export function workLine(summary: RunSummary, durationMs: number | null, tasksDone = 0): string {
  const time = durationMs !== null && durationMs > 0 ? fmtDuration(durationMs) : "";
  const done = tasksDone > 0 ? [`${n(tasksDone, "task")} done`] : [];
  return [time ? `Worked for ${time}` : "Worked", ...summary.parts, ...done].join(" · ");
}

/** What "Copy" takes from a turn: its answer, or every reply when the turn ended on work. */
export function answerText(section: WorkSection): string {
  const texts = (items: TimelineItem[]) => items.flatMap((item) => (item.kind === "text" ? [item.text] : []));
  const answer = texts(section.answer);
  return (answer.length > 0 ? answer : texts(section.work)).join("\n\n");
}
