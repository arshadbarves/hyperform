import type { PacketBlocker } from "../../protocol/PacketBlocker";
import type { PacketReason } from "../../protocol/PacketReason";
import type { TaskPacket } from "../../protocol/TaskPacket";
import type { PacketSent } from "../sessionView/work";
import { MAIN_AGENT } from "../sessionView/types";
import { fmtTokens } from "../../util";
import { entryTitle, evidenceLabel } from "./notebook";
import { blockedText, stateLabel, type TaskMap } from "./status";

/** The newest packet per agent: the main agent first, then helpers, newest first. */
export function latestPackets(packets: readonly PacketSent[]): PacketSent[] {
  const latest = new Map<string, PacketSent>();
  for (const p of packets) {
    const known = latest.get(p.agentId);
    if (!known || known.at <= p.at) latest.set(p.agentId, p);
  }
  return [...latest.values()].sort(
    (a, b) => Number(b.agentId === MAIN_AGENT) - Number(a.agentId === MAIN_AGENT) || b.at - a.at,
  );
}

const SENT: Record<string, string> = {
  turn: "at the turn start",
  taskChanged: "when the task changed",
  compaction: "after compaction",
  resume: "on resume",
  worker: "when the helper started",
};

/** When the packet went out (`taskPacketSent.reason`). */
export function sentReason(reason: string): string {
  return SENT[reason] ?? reason;
}

const REASON: Record<PacketReason, string> = {
  task: "about this task",
  ancestor: "about a parent task",
  dependency: "about a dependency",
  scope: "shares its files",
  recent: "recent",
};

export function packetReason(reason: PacketReason): string {
  return REASON[reason];
}

function blockerText(blocker: PacketBlocker, tasks: TaskMap): string {
  switch (blocker.type) {
    case "blocked":
      return blockedText(blocker.reason, tasks);
    case "waiting":
      return `waits on “${blocker.task.title}”`;
    case "held":
      return "on hold";
    default:
      return `the plan is ${blocker.status}`;
  }
}

export interface PacketLine {
  key: string;
  text: string;
  /** Why it was included, or its status. */
  why: string | null;
  tokens: number | null;
}

export interface PacketSection {
  title: string;
  lines: PacketLine[];
}

/** What the model was given, part by part, with why each entry made it in. */
export function packetSections(packet: TaskPacket, tasks: TaskMap): PacketSection[] {
  const line = (key: string, text: string, why: string | null = null, tokens: number | null = null): PacketLine => ({
    key,
    text,
    why,
    tokens,
  });
  const sections: PacketSection[] = [
    { title: "Goal", lines: [line("goal", packet.goal)] },
    { title: "Invariants", lines: packet.invariants.map((i) => line(i.id, i.text)) },
    { title: "Requirements", lines: packet.requirements.map((r) => line(r.id, r.text)) },
    {
      title: "Task",
      lines: [
        line("task", packet.task.title, packet.parent ? `under “${packet.parent.title}”` : null),
        ...packet.task.acceptance.map((c) => line(`c:${c.id}`, c.text, c.status)),
      ],
    },
    {
      title: "Dependencies",
      lines: packet.dependencies.map((d) => line(d.id, d.title, d.result ?? stateLabel(d.status.type))),
    },
    {
      title: "Notebook",
      lines: packet.entries.map((e) => line(e.entry.id, entryTitle(e.entry), packetReason(e.reason), e.tokens)),
    },
    {
      title: "Evidence",
      lines: packet.evidence.map((e, i) =>
        line(`ev:${i}:${e.criterionId}`, evidenceLabel(e.evidence), e.stale ? "stale" : null),
      ),
    },
    { title: "Blockers", lines: packet.blockers.map((b, i) => line(`b:${i}`, blockerText(b, tasks))) },
    { title: "Scope", lines: packet.scope.map((s) => line(`s:${s}`, s)) },
  ];
  if (packet.resume) sections.push({ title: "Resume from", lines: [line("resume", packet.resume.note)] });
  if (packet.nextReady) sections.push({ title: "Next", lines: [line("next", packet.nextReady.title)] });
  return sections.filter((s) => s.lines.length > 0);
}

/** "3.2k of 4k tokens · 2 left out". */
export function packetSize(packet: Pick<TaskPacket, "tokens" | "budget" | "dropped">): string {
  const size = `${fmtTokens(packet.tokens)} of ${fmtTokens(packet.budget)} tokens`;
  return packet.dropped > 0 ? `${size} · ${packet.dropped} left out` : size;
}
