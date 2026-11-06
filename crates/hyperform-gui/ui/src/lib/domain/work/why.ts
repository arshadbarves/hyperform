import type { Actor } from "../../protocol/Actor";
import type { Entry } from "../../protocol/Entry";
import type { TaskPatch } from "../../protocol/TaskPatch";
import type { TaskStatus } from "../../protocol/TaskStatus";
import type { WhyStep } from "../../protocol/WhyStep";
import type { WorkCause } from "../../protocol/WorkCause";
import type { WorkEvent } from "../../protocol/WorkEvent";
import { MAIN_AGENT } from "../sessionView/types";
import { causeLabel, changeText, entryTitle, evidenceLabel } from "./notebook";
import { stateLabel, type TaskMap } from "./status";

/** One step of a task's history, in words. */
export interface WhyLine {
  key: string;
  revision: number;
  at: number;
  /** "You", "The agent", "A helper", "Automatic", "Compaction". */
  who: string;
  /** What happened: "Created", "Pending → In progress". */
  what: string;
  /** What set it off: "TaskCreate", "your edit", "a plan change". */
  cause: string | null;
  /** The plan change, discovery or experiment behind it. */
  because: string[];
  evidence: string[];
}

export function actorLabel(actor: Actor): string {
  switch (actor.type) {
    case "user":
      return "You";
    case "kernel":
      return "Automatic";
    case "compaction":
      return "Compaction";
    default:
      return actor.agentId === MAIN_AGENT ? "The agent" : "A helper";
  }
}

function causeText(cause: WorkCause): string | null {
  switch (cause.type) {
    case "tool":
      return cause.tool;
    case "edit":
      return "your edit";
    case "approval":
      return "approval";
    case "reconciliation":
      return "reconciliation";
    case "change":
      return "a plan change";
    case "compaction":
      return "compaction";
    default:
      return "resume";
  }
}

function statusWord(status: TaskStatus): string {
  return stateLabel(status.type);
}

const PATCH_FIELDS: readonly [keyof TaskPatch, string][] = [
  ["title", "title"],
  ["description", "description"],
  ["kind", "kind"],
  ["priority", "priority"],
  ["scope", "scope"],
  ["notes", "notes"],
  ["acceptance", "criteria"],
  ["requirements", "requirements"],
  ["runAlone", "run alone"],
  ["result", "result"],
];

function patchText(patch: TaskPatch): string {
  const fields = PATCH_FIELDS.filter(([key]) => patch[key] !== null).map(([, word]) => word);
  return fields.length > 0 ? `Changed ${fields.join(", ")}` : "Edited";
}

function title(tasks: TaskMap, id: string): string {
  return `“${tasks[id]?.title ?? id}”`;
}

export function eventText(event: WorkEvent, tasks: TaskMap): string {
  switch (event.type) {
    case "taskCreated":
      return "Created";
    case "taskEdited":
      return patchText(event.patch);
    case "taskMoved":
      return "Moved";
    case "taskTransitioned":
      return `${statusWord(event.from)} → ${statusWord(event.to)}`;
    case "taskHeld":
      return event.held ? "Put on hold" : "Resumed";
    case "taskBound":
      return event.owner.agentId === MAIN_AGENT ? "Taken by the agent" : "Given to a helper";
    case "taskReleased":
      return "Released";
    case "dependencyAdded":
      return `Now after ${title(tasks, event.dependsOn)}`;
    case "dependencyRemoved":
      return `No longer after ${title(tasks, event.dependsOn)}`;
    case "stepsRecorded":
      return `Steps updated (${event.steps.length})`;
    case "criterionChanged":
      return `Criterion ${event.criterionId}: ${event.status}`;
    case "evidenceLinked":
      return `Evidence: ${evidenceLabel(event.evidence)}`;
    case "entryRecorded":
      return `Recorded: ${entryTitle(event.entry)}`;
    case "planStatusChanged":
      return `Plan ${event.from} → ${event.to}`;
    case "rejected":
      return `Rejected: ${event.reasons.join("; ")}`;
    default:
      return event.type.replace(/([A-Z])/g, " $1").toLowerCase().replace(/^./, (c) => c.toUpperCase());
  }
}

function because(entry: Entry): string | null {
  const body = entry.body;
  switch (body.type) {
    case "planChange":
      return `${causeLabel(body.cause)}: ${changeText(entry)}`;
    case "discovery":
      return `Discovery: ${body.text}`;
    case "experiment":
      return `Experiment: ${body.hypothesis}${body.conclusion ? ` → ${body.conclusion}` : ""}`;
    case "designDecision":
      return `Decision: ${body.choice}${body.reason ? ` (${body.reason})` : ""}`;
    default:
      return null;
  }
}

/** The `why(task)` steps as lines, oldest first. */
export function whyLines(steps: readonly WhyStep[], tasks: TaskMap): WhyLine[] {
  return steps.map((step, i) => ({
    key: `${step.revision}:${i}`,
    revision: step.revision,
    at: step.at,
    who: actorLabel(step.actor),
    what: eventText(step.event, tasks),
    cause: causeText(step.cause),
    because: step.entries.flatMap((e) => {
      const text = because(e);
      return text ? [text] : [];
    }),
    evidence: step.entries.flatMap((e) => (e.body.type === "planChange" ? e.body.evidence.map(evidenceLabel) : [])),
  }));
}
