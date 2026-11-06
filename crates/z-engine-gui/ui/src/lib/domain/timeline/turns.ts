import type { CompactionMarker } from "../../protocol/CompactionMarker";
import type { Message } from "../../protocol/Message";
import type { TaskViewInfo } from "../../protocol/TaskViewInfo";
import type { TurnRecord } from "../../protocol/TurnRecord";
import type { CommandOutputView, ErrorView, RouteView } from "../sessionView/types";
import { planRestored, type PlanTrace } from "../work/trace";
import { hasToolResults, hasVisibleUserContent, visibleText, type ToolUseRef } from "./blocks";

export type TimelineItem =
  | { kind: "text"; key: string; messageId: string; text: string }
  | { kind: "thinking"; key: string; text: string; redacted: boolean }
  | { kind: "tool"; key: string; use: ToolUseRef }
  | { kind: "steer"; key: string; message: Message }
  | { kind: "output"; key: string; output: CommandOutputView }
  | { kind: "error"; key: string; error: ErrorView }
  | { kind: "route"; key: string; route: RouteView }
  /** `restored`: the agent was handed its task packet again after this compaction. */
  | { kind: "compaction"; key: string; marker: CompactionMarker; restored: boolean }
  | { kind: "taskView"; key: string; view: TaskViewInfo; latest: boolean }
  | { kind: "trace"; key: string; trace: PlanTrace };

/** One user prompt and everything the agent did for it. `user` is null for leading content. */
export interface TimelineTurn {
  key: string;
  user: Message | null;
  items: TimelineItem[];
  record: TurnRecord | null;
  active: boolean;
}

export interface TimelineInput {
  messages: Message[];
  turns?: TurnRecord[];
  turnStarts?: Record<string, string>;
  steeringIds?: Record<string, true>;
  activeMessageId?: string | null;
  busy?: boolean;
  outputs?: CommandOutputView[];
  errors?: ErrorView[];
  routes?: RouteView[];
  compactions?: CompactionMarker[];
  taskViews?: TaskViewInfo[];
  /** One-line plan events (`planTraces`), placed after the last message before them. */
  traces?: PlanTrace[];
  /** When the main agent got its task packet back after a compaction (`restoreTimes`). */
  restores?: number[];
}

type Anchors = { before: Map<string, TimelineItem[]>; after: Map<string | null, TimelineItem[]> };

function pushTo<K>(map: Map<K, TimelineItem[]>, key: K, item: TimelineItem) {
  const list = map.get(key);
  if (list) list.push(item);
  else map.set(key, [item]);
}

function anchorExtras(input: TimelineInput): Anchors {
  const before = new Map<string, TimelineItem[]>();
  const after = new Map<string | null, TimelineItem[]>();
  const ids = new Set(input.messages.map((m) => m.id));
  const local = [
    ...(input.outputs ?? []).map((output) => ({
      at: output.id,
      anchor: output.afterMessageId,
      item: { kind: "output", key: `out:${output.id}`, output } as TimelineItem,
    })),
    ...(input.errors ?? []).map((error) => ({
      at: error.id,
      anchor: error.afterMessageId,
      item: { kind: "error", key: `err:${error.id}`, error } as TimelineItem,
    })),
    ...(input.routes ?? []).map((route) => ({
      at: route.id,
      anchor: route.afterMessageId,
      item: { kind: "route", key: `route:${route.id}`, route } as TimelineItem,
    })),
  ].sort((a, b) => a.at - b.at);
  for (const { anchor, item } of local) {
    if (anchor === null || ids.has(anchor)) pushTo(after, anchor, item);
  }
  const compactions = input.compactions ?? [];
  const timed = [
    ...compactions.map((marker, i) => {
      const next = compactions[i + 1]?.createdAt ?? null;
      const restored = planRestored(marker.createdAt, next, input.restores ?? []);
      const item: TimelineItem = { kind: "compaction", key: `compact:${i}:${marker.createdAt}`, marker, restored };
      return { at: marker.createdAt, keepFrom: marker.keepFrom, item };
    }),
    ...(input.traces ?? []).map((trace) => ({
      at: trace.at,
      keepFrom: null,
      item: { kind: "trace", key: trace.key, trace } as TimelineItem,
    })),
  ].sort((a, b) => a.at - b.at);
  for (const { at, keepFrom, item } of timed) {
    if (keepFrom && ids.has(keepFrom)) pushTo(before, keepFrom, item);
    else pushTo(after, input.messages.findLast((m) => m.createdAt <= at)?.id ?? null, item);
  }
  const views = input.taskViews ?? [];
  const lastCompaction = Math.max(0, ...compactions.map((m) => m.createdAt));
  views.forEach((view, i) => {
    if (!ids.has(view.boundary)) return;
    const latest = i === views.length - 1 && lastCompaction < view.createdAt;
    pushTo(before, view.boundary, { kind: "taskView", key: `taskView:${view.boundary}`, view, latest });
  });
  return { before, after };
}

function assistantItems(message: Message): TimelineItem[] {
  const items: TimelineItem[] = [];
  message.content.forEach((block, i) => {
    const key = `${message.id}:${i}`;
    if (block.type === "text" && block.text.trim()) {
      items.push({ kind: "text", key, messageId: message.id, text: block.text });
    } else if (block.type === "thinking" && block.text.trim()) {
      items.push({ kind: "thinking", key, text: block.text, redacted: false });
    } else if (block.type === "redactedThinking") {
      items.push({ kind: "thinking", key, text: "", redacted: true });
    } else if (block.type === "toolUse") {
      items.push({
        kind: "tool",
        key: `tool:${block.id}`,
        use: { callId: block.id, name: block.name, input: block.input },
      });
    }
  });
  return items;
}

/** Group a transcript into turns: prompt -> assistant/tool rounds -> turn record. */
export function buildTimeline(input: TimelineInput): TimelineTurn[] {
  const { messages } = input;
  const records = new Map((input.turns ?? []).map((t) => [t.messageId, t]));
  const starts = input.turnStarts ?? {};
  const steering = input.steeringIds ?? {};
  const activeId = input.activeMessageId ?? null;
  const busy = input.busy ?? false;
  const known = (m: Message) => m.id in starts || records.has(m.id);
  let lastKnownStart = -1;
  messages.forEach((m, i) => {
    if (m.role === "user" && known(m)) lastKnownStart = i;
  });

  const anchors = anchorExtras(input);
  const turns: TimelineTurn[] = [];
  let current: TimelineTurn | null = null;
  const target = (): TimelineTurn => {
    if (current) return current;
    current = { key: "lead", user: null, items: [], record: null, active: false };
    turns.push(current);
    return current;
  };
  const append = (items: TimelineItem[] | undefined) => {
    if (items && items.length > 0) target().items.push(...items);
  };

  function isSteering(m: Message, index: number): boolean {
    if (steering[m.id]) return true;
    if (known(m)) return false;
    const turn = current;
    if (!turn?.user) return false;
    if (turn.record) return m.createdAt <= turn.record.finishedAt;
    if (activeId !== null && turn.user.id === activeId) return true;
    return busy && index > lastKnownStart;
  }

  append(anchors.after.get(null));
  messages.forEach((m, index) => {
    append(anchors.before.get(m.id));
    if (m.role === "assistant") {
      append(assistantItems(m));
    } else if (hasToolResults(m)) {
      if (visibleText(m)) append([{ kind: "steer", key: m.id, message: m }]);
    } else if (hasVisibleUserContent(m)) {
      if (isSteering(m, index)) {
        append([{ kind: "steer", key: m.id, message: m }]);
      } else {
        current = { key: m.id, user: m, items: [], record: records.get(m.id) ?? null, active: false };
        turns.push(current);
      }
    }
    append(anchors.after.get(m.id));
  });

  const last = turns[turns.length - 1];
  for (const turn of turns) {
    if (turn.record || !turn.user) continue;
    turn.active = turn.user.id === activeId || (busy && turn === last);
  }
  return turns;
}
