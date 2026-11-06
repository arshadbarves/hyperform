import type { SessionView } from "./sessionView/types";

export interface ActivityItem {
  key: string;
  tone: "info" | "warn" | "error";
  text: string;
  at: number;
}

type ActivitySource = Pick<SessionView, "notices" | "hooks" | "errors">;

/** Engine notices, blocked hooks and errors, newest first: what a passing toast may have hidden. */
export function recentActivity(view: ActivitySource | null | undefined, limit = 20): ActivityItem[] {
  if (!view) return [];
  const items: ActivityItem[] = [
    ...view.notices.map((n) => ({ key: `n${n.id}`, tone: n.level, text: n.text, at: n.at })),
    ...view.hooks
      .filter((h) => h.blocked)
      .map((h) => ({
        key: `h${h.id}`,
        tone: "warn" as const,
        text: `Hook blocked · ${h.hookEvent}${h.message ? `: ${h.message}` : ""}`,
        at: h.at,
      })),
    ...view.errors.map((e) => ({ key: `e${e.id}`, tone: "error" as const, text: e.message, at: e.at })),
  ];
  return items.sort((a, b) => b.at - a.at).slice(0, limit);
}

/** Only warnings and errors earn the unseen dot; routine notices stay quiet. */
export function hasUnseen(items: ActivityItem[], seenAt: number): boolean {
  return items.some((item) => item.tone !== "info" && item.at > seenAt);
}
