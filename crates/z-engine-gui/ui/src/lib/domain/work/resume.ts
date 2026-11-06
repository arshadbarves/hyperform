import type { Entry } from "../../protocol/Entry";
import type { Task } from "../../protocol/Task";
import { MAIN_AGENT } from "../sessionView/types";

/** The plan change the engine records when it resumes a plan after a restart or an attach. */
export function isResumeChange(entry: Entry): boolean {
  return entry.body.type === "planChange" && entry.body.cause === "resume";
}

/** "2 tasks were interrupted, 1 file changed outside the plan": the engine's summary without its "Resumed:" lead. */
export function resumeSummary(entry: Entry): string {
  if (entry.body.type !== "planChange") return "";
  const text = (entry.body.reason || entry.body.new).trim();
  return text.replace(/^resumed\s*[:·-]\s*/i, "");
}

/** Where an interrupted task picks up again, for its detail. */
export interface ResumeNote {
  text: string;
  /** The engine's own note on where it stopped; null when it left none. */
  note: string | null;
  /** The worktree its helper left, kept for it. */
  worktree: string | null;
  /** The helper whose transcript it picks up from, in the chat that ran it. */
  agentId: string | null;
  sessionId: string;
}

/** A task that was running when the plan stopped and waits to start again; null otherwise. */
export function resumeNote(task: Task): ResumeNote | null {
  const pointer = task.resume;
  if (!pointer || task.status.type !== "pending") return null;
  const helper = pointer.agentId !== MAIN_AGENT;
  const text = pointer.worktree
    ? "Interrupted; its helper's work is kept in a worktree"
    : helper
      ? "Interrupted; it picks up from its helper's transcript"
      : "Interrupted; it picks up where it stopped";
  return {
    text,
    note: pointer.note.trim() || null,
    worktree: pointer.worktree,
    agentId: helper ? pointer.agentId : null,
    sessionId: pointer.sessionId,
  };
}
