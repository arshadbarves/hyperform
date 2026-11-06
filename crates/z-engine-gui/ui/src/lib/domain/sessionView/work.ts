import type { AuditFinding } from "../../protocol/AuditFinding";
import type { Entry } from "../../protocol/Entry";
import type { PlanHeader } from "../../protocol/PlanHeader";
import type { PlanHealth } from "../../protocol/PlanHealth";
import type { PlanStatus } from "../../protocol/PlanStatus";
import type { ReconcileFinding } from "../../protocol/ReconcileFinding";
import type { Task } from "../../protocol/Task";
import type { WorkSnapshot } from "../../protocol/WorkSnapshot";
import type { WorkUpdate } from "../../protocol/WorkUpdate";
import { byKey, capped } from "./records";

/** A plan status move seen live through `workUpdated` (approval, replanning). */
export interface PlanMove {
  revision: number;
  from: PlanStatus;
  to: PlanStatus;
  at: number;
}

/** A task packet the engine sent to an agent (`taskPacketSent`); live only. */
export interface PacketSent {
  agentId: string;
  taskId: string;
  tokens: number;
  reason: string;
  at: number;
}

/** The plan attached to the session. The engine folds the log; the GUI only upserts. */
export interface WorkView {
  plan: PlanHeader;
  tasks: Record<string, Task>;
  entries: Record<string, Entry>;
  /** Findings of the latest reconciliation; each revision brings the current list. */
  findings: ReconcileFinding[];
  /** Every audit finding, dismissed and resolved ones included; replaced by each revision. */
  audit: AuditFinding[];
  /** Counts and levels for the coverage line; null until the engine sends them. */
  health: PlanHealth | null;
  /** While replanning: tasks held until you accept the plan change; other ready tasks keep running. */
  replanHeld: string[];
  /** Status moves seen live, oldest first; a snapshot of the same plan keeps them. */
  moves: PlanMove[];
}

/** Packets kept per session, newest last. */
export const PACKETS_KEPT = 20;

export function fromWorkSnapshot(snap: WorkSnapshot, prev: WorkView | null): WorkView {
  return {
    plan: snap.plan,
    tasks: byKey(snap.tasks, (t) => t.id),
    entries: byKey(snap.entries, (e) => e.id),
    findings: snap.findings ?? [],
    audit: snap.audit ?? [],
    health: snap.health ?? null,
    replanHeld: snap.replanHeld ?? [],
    moves: prev?.plan.planId === snap.plan.planId ? prev.moves : [],
  };
}

function upsertNewer<T extends { revision: number }>(
  record: Record<string, T>,
  items: T[],
  key: (item: T) => string,
): Record<string, T> {
  if (items.length === 0) return record;
  const next = { ...record };
  for (const item of items) {
    const id = key(item);
    const known = next[id];
    if (!known || known.revision <= item.revision) next[id] = item;
  }
  return next;
}

/**
 * One revision: the header, the findings, the audit, health and the tasks
 * held for a plan change (unless an older revision arrives late) and the
 * tasks and entries it changed. A different plan id replaces the view.
 */
export function applyWorkUpdate(prev: WorkView | null, update: WorkUpdate): WorkView {
  const same = prev && prev.plan.planId === update.planId ? prev : null;
  if (!same) {
    return {
      plan: update.plan,
      tasks: byKey(update.tasks, (t) => t.id),
      entries: byKey(update.entries, (e) => e.id),
      findings: update.findings ?? [],
      audit: update.audit ?? [],
      health: update.health ?? null,
      replanHeld: update.replanHeld ?? [],
      moves: [],
    };
  }
  const newer = update.plan.revision >= same.plan.revision;
  const plan = newer ? update.plan : same.plan;
  const moved = newer && plan.status !== same.plan.status;
  return {
    plan,
    tasks: upsertNewer(same.tasks, update.tasks, (t) => t.id),
    entries: upsertNewer(same.entries, update.entries, (e) => e.id),
    findings: newer ? (update.findings ?? same.findings) : same.findings,
    audit: newer ? (update.audit ?? same.audit) : same.audit,
    health: newer ? (update.health ?? same.health) : same.health,
    replanHeld: newer ? (update.replanHeld ?? same.replanHeld) : same.replanHeld,
    moves: moved
      ? [...same.moves, { revision: plan.revision, from: same.plan.status, to: plan.status, at: plan.updatedAt }]
      : same.moves,
  };
}

export function addPacket(packets: PacketSent[], packet: PacketSent): PacketSent[] {
  return capped(packets, packet, PACKETS_KEPT);
}
