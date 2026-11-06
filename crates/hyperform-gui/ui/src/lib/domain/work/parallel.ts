import type { FeatureMode } from "../../protocol/config/FeatureMode";
import type { ParallelBlock } from "../../protocol/ParallelBlock";
import type { ParallelSet } from "../../protocol/ParallelSet";
import { workPlansOn } from "./panel";
import type { TaskMap } from "./status";

/** The feature flag that lets independent tasks run in parallel helpers. */
export const WORK_PARALLEL_FEATURE = "work_parallel";

/** Effective only while work plans run too: the engine keeps the setting but leaves it off. */
export function workParallelOn(experimental: Readonly<Record<string, FeatureMode>> | null | undefined): boolean {
  const mode = experimental?.[WORK_PARALLEL_FEATURE];
  return workPlansOn(experimental) && mode !== undefined && mode !== "off";
}

function title(tasks: TaskMap, id: string): string {
  return `“${tasks[id]?.title ?? id}”`;
}

/** A parallel block in a sentence, for a tooltip; `self` is the task it is about. */
export function describeBlock(block: ParallelBlock, tasks: TaskMap, self: string): string {
  switch (block.type) {
    case "planning":
      return "Plan mode is on; helpers start once you build";
    case "notApproved":
      return block.parallel === "ask" ? "Parallel helpers wait for your approval" : "Parallel helpers are off for this plan";
    case "runAlone":
      if (block.taskId !== self) return `${title(tasks, block.taskId)} runs alone, so nothing starts beside it`;
      return tasks[self]?.run.serial ? "Retried one at a time after a merge conflict" : "Set to run alone";
    case "unknownScope":
      return block.taskId === self
        ? "It names no files, so an overlap cannot be ruled out"
        : `${title(tasks, block.taskId)} names no files, so nothing starts beside it`;
    case "serialPaths":
      return block.taskId === self
        ? `It touches ${block.patterns.join(", ")}, which never change in parallel`
        : `${title(tasks, block.taskId)} touches ${block.patterns.join(", ")}, so nothing starts beside it`;
    case "ordered":
      return `A dependency links it to ${title(tasks, block.taskId)}`;
    case "overlap":
      return `It shares ${[...block.files, ...block.symbols].join(", ") || "scope"} with ${title(tasks, block.taskId)}`;
    case "noSlot":
      return `${block.max} helpers already run, the most at once (work.max_parallel)`;
    case "replanning":
      return "Held until you accept the plan change";
  }
}

/** A row's quiet parallel badge: it can run beside others, or why it runs alone. */
export interface ParallelBadge {
  text: string;
  title: string;
  together: boolean;
}

/**
 * The short form; null for what holds the whole plan back (plan mode,
 * approval) or a pending plan change, which the plan's own controls and
 * the hold badge say.
 */
function blockBadge(block: ParallelBlock, tasks: TaskMap, self: string): string | null {
  switch (block.type) {
    case "planning":
    case "notApproved":
    case "replanning":
      return null;
    case "runAlone":
    case "unknownScope":
    case "serialPaths":
      return block.taskId === self ? "runs alone" : `not beside ${tasks[block.taskId]?.title ?? block.taskId}`;
    case "ordered":
      return `ordered with ${tasks[block.taskId]?.title ?? block.taskId}`;
    case "overlap":
      return `overlaps ${tasks[block.taskId]?.title ?? block.taskId}`;
    case "noSlot":
      return "waits for a helper";
  }
}

/** Tasks that may run at once: those eligible plus those already running. */
function together(set: ParallelSet): number {
  return set.eligible.length + set.running.length;
}

/** Badges by task id. Eligible tasks say so only when at least two would run at once. */
export function parallelBadges(set: ParallelSet | null, tasks: TaskMap): Record<string, ParallelBadge> {
  if (!set) return {};
  const out: Record<string, ParallelBadge> = {};
  const n = together(set);
  if (n >= 2) {
    const title = `Can run in a parallel helper beside ${n - 1} other task${n === 2 ? "" : "s"}`;
    for (const id of set.eligible) out[id] = { text: "can run beside others", title, together: true };
  }
  for (const { taskId, block } of set.excluded) {
    const text = blockBadge(block, tasks, taskId);
    if (text) out[taskId] = { text, title: describeBlock(block, tasks, taskId), together: false };
  }
  return out;
}

/** "N tasks can run together" for the Draft's Build menu; 0 when fewer than two could. */
export function parallelCount(set: ParallelSet | null): number {
  if (!set) return 0;
  const n = together(set);
  return n >= 2 ? n : 0;
}
