import type { PlanSummary } from "../protocol/PlanSummary";
import type { Urgency } from "../protocol/Urgency";
import { recentActivity } from "./activity";
import { unreadSessionOutcome } from "./sessionOutcome";
import type { SessionActivity, UnreadMark } from "./sessions";
import type { SessionView } from "./sessionView/types";

/** Something a chat (or a work plan) is blocked on until the user answers. */
export interface NeedsYouItem {
  key: string;
  /** Empty for a work plan whose last chat is gone. */
  sessionId: string;
  chatTitle: string;
  kind: "approval" | "question" | "plan" | "trust" | "workPlan";
  title: string;
  detail: string | null;
  /** Set for approvals, which the inbox can answer in place. */
  requestId: string | null;
  /** How urgent the decision model rated it (`decisions_inbox_priority`); null when not rated. */
  urgency: Urgency | null;
  /** Set for a work plan that waits on you, which opens in its chat's Plan tab. */
  plan?: PlanSummary;
}

export interface FinishedItem {
  sessionId: string;
  chatTitle: string;
  label: string;
  tone: "ok" | "neutral" | "danger";
  at: number;
}

/** A passing notice (toast) as the inbox keeps it: in full. */
export interface RecordedNotice {
  key: string;
  tone: "info" | "ok" | "warn" | "error";
  title: string;
  text: string;
  sessionId: string | null;
  at: number;
}

export interface InboxNotice extends RecordedNotice {
  chatTitle: string | null;
  /** How urgent the decision model rated it (`decisions_inbox_priority`); null when not rated. */
  urgency: Urgency | null;
}

type TitleOf = (sessionId: string) => string;

const URGENCY_RANK: Record<Urgency, number> = { high: 0, normal: 1, low: 2 };

/** Most urgent first; unrated items count as normal and ties keep their order. */
export function byUrgency<T extends { urgency: Urgency | null }>(items: T[]): T[] {
  const rank = (item: T) => URGENCY_RANK[item.urgency ?? "normal"];
  return items
    .map((item, index) => ({ item, index }))
    .sort((a, b) => rank(a.item) - rank(b.item) || a.index - b.index)
    .map(({ item }) => item);
}

/** The engine keys a notice's urgency by its text. */
function noticeUrgency(view: SessionView | undefined, text: string): Urgency | null {
  return view?.urgency[`notice:${text}`] ?? null;
}

export function needsYou(views: Record<string, SessionView>, titleOf: TitleOf): NeedsYouItem[] {
  const items: NeedsYouItem[] = [];
  for (const view of Object.values(views)) {
    const id = view.sessionId;
    const base = { sessionId: id, chatTitle: titleOf(id) };
    const urgency = (requestId: string) => view.urgency[requestId] ?? null;
    for (const a of Object.values(view.approvals)) {
      items.push({ ...base, key: `${id}:${a.requestId}`, kind: "approval", title: a.title, detail: a.reason || null, requestId: a.requestId, urgency: urgency(a.requestId) });
    }
    for (const q of Object.values(view.questions)) {
      const first = q.questions[0];
      items.push({ ...base, key: `${id}:${q.requestId}`, kind: "question", title: first?.question ?? "A question for you", detail: null, requestId: null, urgency: urgency(q.requestId) });
    }
    for (const p of Object.values(view.plans)) {
      items.push({ ...base, key: `${id}:${p.requestId}`, kind: "plan", title: "Plan ready for review", detail: null, requestId: null, urgency: urgency(p.requestId) });
    }
    if (view.trustRequest) {
      items.push({
        ...base,
        key: `${id}:trust`,
        kind: "trust",
        title: "Trust this project?",
        detail: view.trustRequest.defines.length ? `Held back: ${view.trustRequest.defines.join(", ")}` : null,
        requestId: null,
        urgency: null,
      });
    }
  }
  return byUrgency(items);
}

const FINISHED_TONE = { verified: "ok", warn: "danger", neutral: "neutral" } as const;

/** Chats whose turn ended in the background and were not opened since, newest first. */
export function finishedChats(
  unread: Record<string, UnreadMark>,
  activity: Record<string, SessionActivity>,
  titleOf: TitleOf,
): FinishedItem[] {
  const items: FinishedItem[] = [];
  for (const [sessionId, mark] of Object.entries(unread)) {
    const outcome = unreadSessionOutcome(mark, false, activity[sessionId] ?? null);
    if (!outcome) continue;
    items.push({ sessionId, chatTitle: titleOf(sessionId), label: outcome.label, tone: FINISHED_TONE[outcome.tone], at: mark.at });
  }
  return items.sort((a, b) => b.at - a.at);
}

/** Every notice worth keeping: passing ones plus each chat's warnings, blocked hooks and errors, most urgent then newest first. */
export function inboxNotices(recorded: RecordedNotice[], views: Record<string, SessionView>, titleOf: TitleOf): InboxNotice[] {
  const fromChats: InboxNotice[] = Object.values(views).flatMap((view) =>
    recentActivity(view, 50).map((item) => ({
      key: `${view.sessionId}:${item.key}`,
      tone: item.tone,
      title: item.text,
      text: item.text,
      sessionId: view.sessionId,
      chatTitle: titleOf(view.sessionId),
      at: item.at,
      urgency: item.key.startsWith("n") ? noticeUrgency(view, item.text) : null,
    })),
  );
  const passing = recorded.map((n) => ({
    ...n,
    chatTitle: n.sessionId ? titleOf(n.sessionId) : null,
    urgency: n.sessionId ? noticeUrgency(views[n.sessionId], n.text) : null,
  }));
  return byUrgency([...fromChats, ...passing].sort((a, b) => b.at - a.at));
}

/** The sidebar's Inbox badge: what needs you, results not yet seen, and problems since the inbox was last opened. */
export function inboxCount(input: { needsYou: number; finished: number; notices: InboxNotice[]; readAt: number }): number {
  const problems = input.notices.filter((n) => (n.tone === "warn" || n.tone === "error") && n.at > input.readAt).length;
  return input.needsYou + input.finished + problems;
}
