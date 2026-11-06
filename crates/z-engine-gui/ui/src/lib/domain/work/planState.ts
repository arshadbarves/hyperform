import type { Criterion } from "../../protocol/Criterion";
import type { WorkView } from "../sessionView/work";
import { hasBound, type PanelKind } from "./panel";
import { resumeNote } from "./resume";

/** The calm banner over a paused, done or abandoned plan, and what it offers. */
export interface StateBanner {
  kind: "paused" | "done" | "abandoned";
  title: string;
  text: string;
  /** Done: what was finished and how it was verified, one line each. */
  lines: string[];
  resume: boolean;
  /** Paused with helpers still winding down: stop them now. */
  stop: boolean;
  reopen: boolean;
}

const CHECKS_NAMED = 4;

function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

function metBy(criteria: Criterion[]): string | null {
  const met = criteria.filter((c) => c.status === "met");
  if (!met.length) return null;
  const by = (method: Criterion["method"]["type"]) => met.filter((c) => c.method.type === method).length;
  const parts = [
    by("check") && `${by("check")} by checks`,
    by("inspection") && `${by("inspection")} by inspection`,
    by("user") && `${by("user")} by you`,
  ].filter(Boolean);
  return `${count(met.length, "criterion", "criteria")} met: ${parts.join(", ")}`;
}

/** The checks whose passing results back met criteria, by label, at most a few. */
function passedChecks(criteria: Criterion[]): string | null {
  const labels = new Set<string>();
  for (const c of criteria) {
    if (c.status !== "met") continue;
    for (const e of c.evidence) if (e.type === "check" && e.passed && e.label) labels.add(e.label);
  }
  if (!labels.size) return null;
  const named = [...labels].slice(0, CHECKS_NAMED);
  const more = labels.size - named.length;
  return `Checks passed: ${named.join(", ")}${more > 0 ? ` and ${more} more` : ""}`;
}

/** What a done plan finished and how its criteria were shown to hold. */
export function doneLines(work: WorkView): string[] {
  const tasks = Object.values(work.tasks);
  const done = tasks.filter((t) => t.status.type === "done");
  const notNeeded = tasks.filter((t) => t.status.type === "cancelled").length;
  const criteria = done.flatMap((t) => t.acceptance);
  const waived = criteria.filter((c) => c.status === "waived").length;
  return [
    [`${count(done.length, "task")} done`, notNeeded && `${notNeeded} not needed`].filter(Boolean).join(" · "),
    metBy(criteria),
    passedChecks(criteria),
    waived ? `${count(waived, "criterion", "criteria")} waived by you` : null,
  ].filter((line): line is string => !!line);
}

/** Interrupted tasks waiting to pick up where they stopped. */
function interrupted(work: WorkView): number {
  return Object.values(work.tasks).filter((t) => resumeNote(t) !== null).length;
}

/** The banner for a paused, done or abandoned plan; null in every other state. */
export function stateBanner(kind: PanelKind, work: WorkView): StateBanner | null {
  switch (kind) {
    case "paused": {
      const bound = hasBound(work);
      const waiting = interrupted(work);
      const text = bound
        ? "Nothing new starts. Running helpers stop at their next step and keep their work."
        : waiting
          ? `Nothing runs until you resume; ${count(waiting, "interrupted task")} ${waiting === 1 ? "picks" : "pick"} up where ${waiting === 1 ? "it" : "they"} stopped.`
          : "Nothing runs until you resume. The work so far is kept.";
      return { kind, title: "Paused", text, lines: [], resume: true, stop: bound, reopen: false };
    }
    case "done":
      return {
        kind,
        title: "Done",
        text: "Every task finished and its criteria hold.",
        lines: doneLines(work),
        resume: false,
        stop: false,
        reopen: true,
      };
    case "abandoned":
      return {
        kind,
        title: "Abandoned",
        text: "Read-only. Nothing runs on this plan; reopen it to pick it up again.",
        lines: [],
        resume: false,
        stop: false,
        reopen: true,
      };
    default:
      return null;
  }
}
