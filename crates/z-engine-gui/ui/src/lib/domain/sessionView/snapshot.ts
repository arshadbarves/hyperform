import type { SessionSnapshot } from "../../protocol/SessionSnapshot";
import { toolUseIds } from "../timeline/blocks";
import { byKey, filterRecord } from "./records";
import { emptyView, MAIN_AGENT, type SessionView } from "./types";
import { fromWorkSnapshot } from "./work";

/**
 * A snapshot replaces the view. Live-only state the snapshot cannot carry
 * (tool durations and progress, in-flight streams, a running compaction,
 * local cards) survives when it still refers to something in the new
 * transcript or the session is still busy.
 */
export function fromSnapshot(prev: SessionView, snap: SessionSnapshot): SessionView {
  const busy = snap.status !== "idle";
  const messageIds = new Set(snap.messages.map((m) => m.id));
  const callIds = toolUseIds(snap.messages);
  const agents = byKey(snap.agents, (a) => a.agentId);
  const anchored = (id: string | null) => id === null || messageIds.has(id);

  const turnStarts: Record<string, string> = {
    ...filterRecord(prev.turnStarts, (_, id) => messageIds.has(id)),
  };
  for (const turn of snap.turns) turnStarts[turn.messageId] = turn.turnId;
  const todos = filterRecord(prev.todos, (_, agentId) => agentId !== MAIN_AGENT && agentId in agents);

  return {
    ...emptyView(prev.sessionId),
    lastSeq: prev.lastSeq,
    info: snap.info,
    status: snap.status,
    messages: snap.messages,
    streaming: busy ? prev.streaming : {},
    tools: filterRecord(
      prev.tools,
      (t) => callIds.has(t.callId) || t.agentId in agents || (busy && t.status === "running"),
    ),
    approvals: byKey(snap.pendingApprovals, (a) => a.requestId),
    questions: byKey(snap.pendingQuestions, (q) => q.requestId),
    plans: byKey(snap.pendingPlans, (p) => p.requestId),
    todos: { ...todos, [MAIN_AGENT]: snap.todos },
    agents,
    jobs: byKey(snap.jobs, (j) => j.jobId),
    checks: snap.checks,
    turns: snap.turns,
    turnStarts,
    steeringIds: filterRecord(prev.steeringIds, (_, id) => messageIds.has(id)),
    activeTurn: busy && prev.activeTurn && messageIds.has(prev.activeTurn.messageId) ? prev.activeTurn : null,
    checkpoints: snap.checkpoints,
    compactions: snap.compactions,
    taskViews: snap.taskViews,
    compacting: busy && prev.compacting,
    usage: snap.usage,
    agentUsage: prev.agentUsage,
    costUsd: snap.costUsd,
    contextTokens: snap.contextTokens,
    contextLimit: snap.contextLimit,
    contextBreakdown: prev.contextBreakdown,
    queue: snap.queued,
    mode: snap.info.mode,
    planning: snap.info.planning,
    model: snap.info.model,
    effort: snap.info.effort,
    title: snap.info.title,
    verification: busy ? prev.verification : null,
    claims: prev.claims,
    suggestions: prev.suggestions,
    turnTones: prev.turnTones,
    urgency: prev.urgency,
    work: snap.work ? fromWorkSnapshot(snap.work, prev.work) : null,
    packets: snap.work ? prev.packets : [],
    notices: prev.notices,
    hooks: prev.hooks,
    outputs: prev.outputs.filter((o) => anchored(o.afterMessageId)),
    errors: prev.errors.filter((e) => anchored(e.afterMessageId)),
    routes: prev.routes.filter((r) => anchored(r.afterMessageId)),
    lastError: prev.lastError,
    retrying: busy ? prev.retrying : null,
    nextLocalId: prev.nextLocalId,
  };
}
