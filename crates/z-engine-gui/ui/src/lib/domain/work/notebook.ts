import type { ChangeCause } from "../../protocol/ChangeCause";
import type { Entry } from "../../protocol/Entry";
import type { EvidenceRef } from "../../protocol/EvidenceRef";
import type { ExperimentOutcome } from "../../protocol/ExperimentOutcome";
import type { Provenance } from "../../protocol/Provenance";
import { isResumeChange, resumeSummary } from "./resume";
import type { TaskMap } from "./status";

export type NotebookTab = "decisions" | "discoveries" | "experiments" | "changes";

export const NOTEBOOK_TABS: readonly { value: NotebookTab; label: string; description: string }[] = [
  { value: "decisions", label: "Decisions", description: "Design decisions and why" },
  { value: "discoveries", label: "Discoveries", description: "Facts found along the way" },
  { value: "experiments", label: "Experiments", description: "Hypotheses tried and what came of them" },
  { value: "changes", label: "Changes", description: "Changes of plan, with cause and evidence" },
];

export type EntryMap = Readonly<Record<string, Entry>>;

export interface EntryFact {
  label: string;
  value: string;
}

/** One notebook line and what it expands to. */
export interface NotebookLine {
  id: string;
  title: string;
  /** A short tag after the title: an experiment's outcome, a change's cause. */
  tag: string | null;
  facts: EntryFact[];
  /** Proposed, not yet confirmed: extracted during compaction or proposed by an agent. */
  unconfirmed: boolean;
  /** Recorded by a turn that read web content; stays marked once confirmed. */
  fromWeb: boolean;
  source: string | null;
  at: number;
}

export function notebookTab(entry: Entry): NotebookTab | null {
  switch (entry.body.type) {
    case "designDecision":
      return "decisions";
    case "discovery":
      return "discoveries";
    case "experiment":
      return "experiments";
    case "planChange":
      return "changes";
    default:
      return null;
  }
}

/** Dismissed and superseded entries leave the notebook. */
export function shownEntry(entry: Entry): boolean {
  return entry.status.type === "proposed" || entry.status.type === "accepted";
}

/** Entries per tab, newest first. */
export function notebook(entries: EntryMap): Record<NotebookTab, Entry[]> {
  const out: Record<NotebookTab, Entry[]> = { decisions: [], discoveries: [], experiments: [], changes: [] };
  for (const entry of Object.values(entries)) {
    const tab = notebookTab(entry);
    if (tab && shownEntry(entry)) out[tab].push(entry);
  }
  for (const list of Object.values(out)) list.sort((a, b) => b.createdAt - a.createdAt);
  return out;
}

/** "Decisions 3 · Discoveries 5 · Changes 6"; empty tabs are left out. */
export function notebookSummary(groups: Record<NotebookTab, Entry[]>): string {
  const parts = NOTEBOOK_TABS.filter((t) => groups[t.value].length > 0).map((t) => `${t.label} ${groups[t.value].length}`);
  return parts.length > 0 ? parts.join(" · ") : "Nothing recorded yet";
}

const OUTCOME: Record<ExperimentOutcome, string> = {
  open: "Open",
  confirmed: "Confirmed",
  refuted: "Refuted",
  inconclusive: "Inconclusive",
};

const CAUSE: Record<ChangeCause, string> = {
  user: "You",
  discovery: "Discovery",
  experiment: "Experiment",
  reconciliation: "Reconciliation",
  workerReport: "Helper report",
  replan: "Replanning",
  resume: "Resumed",
};

const PROVENANCE: Record<Provenance, string> = {
  user: "Added by you",
  agent: "Proposed by the agent",
  research: "From web research",
  compaction: "Extracted while compacting",
};

export function causeLabel(cause: ChangeCause): string {
  return CAUSE[cause];
}

export function evidenceLabel(ref: EvidenceRef): string {
  switch (ref.type) {
    case "check":
      return `${ref.label} ${ref.passed ? "passed" : "failed"}`;
    case "diff":
      return ref.summary || ref.files.join(", ");
    case "note":
      return ref.text;
    default:
      return ref.title || ref.url;
  }
}

function fact(label: string, value: string | null | undefined): EntryFact[] {
  return value && value.trim() ? [{ label, value }] : [];
}

/** A plan change's reason; for a resume, what resuming found without the "Resumed:" lead its cause tag carries. */
export function changeText(entry: Entry): string {
  if (entry.body.type !== "planChange") return "";
  return isResumeChange(entry) ? resumeSummary(entry) : entry.body.reason || entry.body.new;
}

function resumeTitle(entry: Entry): string {
  const text = changeText(entry);
  return text ? text[0].toUpperCase() + text.slice(1) : "Plan resumed";
}

/** An entry's one-line title. */
export function entryTitle(entry: Entry): string {
  const body = entry.body;
  switch (body.type) {
    case "designDecision":
      return body.title || body.choice;
    case "discovery":
      return body.text;
    case "experiment":
      return body.hypothesis;
    case "planChange":
      return isResumeChange(entry) ? resumeTitle(entry) : changeText(entry);
    default:
      return evidenceLabel(body.evidence);
  }
}

export function entryLine(entry: Entry, tasks: TaskMap): NotebookLine {
  const body = entry.body;
  let tag: string | null = null;
  let facts: EntryFact[] = [];
  if (body.type === "designDecision") {
    facts = [...fact("Choice", body.choice), ...fact("Alternatives", body.alternatives.join(" · ")), ...fact("Reason", body.reason)];
  } else if (body.type === "discovery") {
    facts = fact("Source", body.source);
  } else if (body.type === "experiment") {
    tag = OUTCOME[body.outcome];
    facts = [...fact("Method", body.method), ...fact("Result", body.result), ...fact("Conclusion", body.conclusion)];
  } else if (body.type === "planChange") {
    tag = CAUSE[body.cause];
    const affected = body.affected.map((id) => tasks[id]?.title ?? id).join(" · ");
    facts = [
      ...fact("From", body.original),
      ...fact("To", body.new),
      ...fact("Evidence", body.evidence.map(evidenceLabel).join(" · ")),
      ...fact("Tasks", affected),
    ];
  }
  const unconfirmed = entry.status.type === "proposed";
  return {
    id: entry.id,
    title: entryTitle(entry),
    tag,
    facts,
    unconfirmed,
    fromWeb: entry.provenance === "research",
    source: unconfirmed ? PROVENANCE[entry.provenance] : null,
    at: entry.createdAt,
  };
}

/** Design decisions, discoveries and experiments that touch a task, newest first. */
export function linkedEntries(entries: EntryMap, taskId: string): Entry[] {
  return Object.values(entries)
    .filter((e) => {
      const tab = notebookTab(e);
      return tab !== null && tab !== "changes" && shownEntry(e) && e.tasks.includes(taskId);
    })
    .sort((a, b) => b.createdAt - a.createdAt);
}
