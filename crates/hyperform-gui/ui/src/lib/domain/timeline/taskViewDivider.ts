import type { TaskViewInfo } from "../../protocol/TaskViewInfo";
import { fmtTokens } from "../../util";

/** The words of a task-view divider and whether it offers "Include full history". */
export interface TaskViewDividerText {
  label: string;
  canRestore: boolean;
}

/**
 * Only the newest view can be undone: the engine keeps one full history,
 * until the next task boundary.
 */
export function taskViewDivider(view: TaskViewInfo, latest: boolean): TaskViewDividerText {
  if (view.restored) return { label: "Full history included for this task", canRestore: false };
  const noun = view.setAside === 1 ? "exchange" : "exchanges";
  return {
    label: `${view.setAside} earlier ${noun} (${fmtTokens(view.tokens)} tokens) set aside for this task`,
    canRestore: latest,
  };
}
