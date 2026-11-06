import type { Message } from "../protocol/Message";
import type { SessionSummary } from "../protocol/SessionSummary";
import type { TurnOutcome } from "../protocol/TurnOutcome";
import type { SessionView } from "./sessionView/types";
import { hasToolResults, visibleText } from "./timeline/blocks";

/** One sidebar / palette row, merged from `list_sessions` and live views. */
export interface SessionListItem {
  sessionId: string;
  title: string;
  projectRoot: string;
  updatedAt: number;
  legacy: boolean;
  costUsd: number;
  lastOutcome: TurnOutcome | null;
}

export const UNTITLED = "New chat";

/** First non-empty line, clipped to 48 characters. */
export function fallbackTitle(prompt: string): string {
  const line =
    prompt
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l.length > 0) ?? prompt.trim();
  const chars = [...line];
  if (chars.length <= 48) return chars.join("");
  return `${chars.slice(0, 48).join("")}…`;
}

export function sessionLabel(title: string | null | undefined): string {
  const t = title?.trim();
  return t ? t : UNTITLED;
}

export function firstUserText(messages: Message[]): string | null {
  for (const m of messages) {
    if (m.role !== "user" || hasToolResults(m)) continue;
    const text = visibleText(m).trim();
    if (text) return text;
  }
  return null;
}

/** Display title for a session: engine title, else the first prompt. */
export function viewTitle(view: SessionView | undefined, summary?: SessionSummary): string | null {
  const title = view?.title ?? view?.info?.title ?? summary?.title ?? null;
  if (title?.trim()) return title.trim();
  const first = view ? firstUserText(view.messages) : null;
  return first ? fallbackTitle(first) : null;
}

function lastActivity(view: SessionView | undefined): number {
  if (!view) return 0;
  const last = view.messages[view.messages.length - 1]?.createdAt ?? 0;
  return Math.max(last, view.info?.updatedAt ?? 0);
}

/**
 * Merge the disk listing with sessions opened this run. Sessions with no
 * messages stay hidden until their first prompt; the list is newest first.
 */
export function listItems(
  summaries: SessionSummary[],
  views: Record<string, SessionView>,
): SessionListItem[] {
  const out = new Map<string, SessionListItem>();
  for (const s of summaries) {
    const view = views[s.sessionId];
    const hasMessages = s.messageCount > 0 || (view?.messages.length ?? 0) > 0;
    if (!hasMessages) continue;
    out.set(s.sessionId, {
      sessionId: s.sessionId,
      title: sessionLabel(viewTitle(view, s)),
      projectRoot: view?.info?.projectRoot ?? s.projectRoot,
      updatedAt: Math.max(s.updatedAt, lastActivity(view)),
      legacy: s.legacy,
      costUsd: view?.info ? view.costUsd : s.costUsd,
      lastOutcome: s.lastOutcome,
    });
  }
  for (const [id, view] of Object.entries(views)) {
    if (out.has(id) || !view.info || view.messages.length === 0) continue;
    out.set(id, {
      sessionId: id,
      title: sessionLabel(viewTitle(view)),
      projectRoot: view.info.projectRoot,
      updatedAt: lastActivity(view),
      legacy: view.info.legacy,
      costUsd: view.costUsd,
      lastOutcome: view.turns[view.turns.length - 1]?.outcome ?? null,
    });
  }
  return [...out.values()].sort((a, b) => b.updatedAt - a.updatedAt);
}

function sameItem(a: SessionListItem, b: SessionListItem): boolean {
  return (
    a.title === b.title &&
    a.projectRoot === b.projectRoot &&
    a.updatedAt === b.updatedAt &&
    a.legacy === b.legacy &&
    a.costUsd === b.costUsd &&
    a.lastOutcome === b.lastOutcome
  );
}

/** Reuse unchanged rows (and the list itself) so keyed lists skip re-rendering. */
export function reuseItems(prev: SessionListItem[], next: SessionListItem[]): SessionListItem[] {
  const byId = new Map(prev.map((item) => [item.sessionId, item]));
  let changed = prev.length !== next.length;
  const out = next.map((item, i) => {
    const old = byId.get(item.sessionId);
    const kept = old && sameItem(old, item) ? old : item;
    if (kept !== prev[i]) changed = true;
    return kept;
  });
  return changed ? out : prev;
}

/** True when two flat string records hold the same entries. */
export function sameRecord<T>(a: Record<string, T>, b: Record<string, T>): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((k) => a[k] === b[k]);
}
