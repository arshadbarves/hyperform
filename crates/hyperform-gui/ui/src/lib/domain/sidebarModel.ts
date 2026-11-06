import { relTime } from "../util";
import { sameWorkspacePath, wsBasename } from "./paths";
import type { SessionListItem } from "./sessionList";
import { sidebarMark, type SidebarMark } from "./sessionOutcome";
import type { SessionActivity, UnreadMark } from "./sessions";
import type { SessionView } from "./sessionView/types";

/** Chats a folded project lists before "Show more". */
export const CHATS_SHOWN = 8;
/** Chats outside any project listed before "Show more". */
export const OTHERS_SHOWN = 12;

export interface ChatRow {
  item: SessionListItem;
  active: boolean;
  mark: SidebarMark | null;
  /** Age of the last activity: `now`, `5m`, `3h`, `2d`. */
  when: string;
}

export interface ProjectGroup {
  root: string;
  name: string;
  active: boolean;
  rows: ChatRow[];
  hidden: number;
  /** The most urgent live mark of its chats, shown while the project is folded. */
  mark: SidebarMark | null;
}

export interface SidebarInput {
  sessions: SessionListItem[];
  roots: string[];
  activeRoot: string | null;
  activeSessionId: string | null;
  activity: Record<string, SessionActivity>;
  unread: Record<string, UnreadMark>;
  /** Projects (by root, or `others`) whose chats are all shown. */
  expanded: Record<string, boolean>;
  now: number;
}

export interface SidebarModel {
  projects: ProjectGroup[];
  others: ChatRow[];
  othersHidden: number;
}

function row(item: SessionListItem, input: SidebarInput): ChatRow {
  const active = item.sessionId === input.activeSessionId;
  return {
    item,
    active,
    mark: sidebarMark({
      active,
      activity: input.activity[item.sessionId] ?? null,
      unread: input.unread[item.sessionId],
      lastOutcome: item.lastOutcome,
    }),
    when: relTime(item.updatedAt, input.now),
  };
}

/** The first `limit` rows, plus the open chat wherever it is. */
function visible(rows: ChatRow[], limit: number, all: boolean): { rows: ChatRow[]; hidden: number } {
  if (all || rows.length <= limit) return { rows, hidden: 0 };
  const shown = rows.slice(0, limit);
  const active = rows.slice(limit).find((r) => r.active);
  if (active) shown.push(active);
  return { rows: shown, hidden: rows.length - shown.length };
}

function groupMark(items: SessionListItem[], activity: Record<string, SessionActivity>): SidebarMark | null {
  const live = items.map((s) => activity[s.sessionId]).filter(Boolean);
  if (live.includes("approval")) return { tone: "attention", label: "A chat here needs you" };
  return live.length ? { tone: "working", label: "Working" } : null;
}

/** Projects with their chats, and chats whose folder is not a registered project. */
export function sidebarModel(input: SidebarInput): SidebarModel {
  const byRoot = new Map<string, SessionListItem[]>(input.roots.map((root) => [root, []]));
  const others: SessionListItem[] = [];
  for (const s of input.sessions) {
    const root = s.projectRoot ? input.roots.find((r) => sameWorkspacePath(s.projectRoot, r)) : undefined;
    if (root) byRoot.get(root)!.push(s);
    else others.push(s);
  }
  const projects = input.roots.map((root) => {
    const items = byRoot.get(root) ?? [];
    const shown = visible(
      items.map((s) => row(s, input)),
      CHATS_SHOWN,
      Boolean(input.expanded[root]),
    );
    return {
      root,
      name: wsBasename(root),
      active: sameWorkspacePath(input.activeRoot, root),
      rows: shown.rows,
      hidden: shown.hidden,
      mark: groupMark(items, input.activity),
    };
  });
  const rest = visible(
    others.map((s) => row(s, input)),
    OTHERS_SHOWN,
    Boolean(input.expanded.others),
  );
  return { projects, others: rest.rows, othersHidden: rest.hidden };
}

/** Finished turns per project of the live chats: a change means the project's files may have changed. */
export function turnsByProject(views: Record<string, SessionView>): Record<string, number> {
  const out: Record<string, number> = {};
  for (const view of Object.values(views)) {
    const root = view.info?.projectRoot;
    if (root) out[root] = (out[root] ?? 0) + view.turns.length;
  }
  return out;
}
