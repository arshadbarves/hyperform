import type { Event } from "../../protocol/Event";
import type { SessionInfo } from "../../protocol/SessionInfo";
import { addCommandOutput, addError, addHookRun, addNotice } from "./notes";
import { drop, put, upsertBy } from "./records";
import { addRoute } from "./routes";
import { fromSnapshot } from "./snapshot";
import { appendDelta, finishAssistant, startStreaming } from "./streaming";
import { addSuggestion, dropSuggestion } from "./suggestions";
import { finishTool, progressTool, startTool } from "./tools";
import { onTurnFinished, onTurnStarted, onUserMessage, withStatus } from "./turns";
import { MAIN_AGENT, type SessionView } from "./types";
import { addPacket, applyWorkUpdate } from "./work";

function patchInfo(info: SessionInfo | null, patch: Partial<SessionInfo>): SessionInfo | null {
  return info ? { ...info, ...patch } : null;
}

/** Pure event reducer over one session. `now` stamps live timers (elapsed, retry countdown). */
export function reduce(view: SessionView, event: Event, now: number): SessionView {
  switch (event.type) {
    case "snapshot":
      return fromSnapshot(view, event.snapshot);
    case "statusChanged":
      return withStatus(view, event.status);
    case "userMessage":
      return onUserMessage(view, event.message, event.turnId, event.steering);
    case "turnStarted":
      return onTurnStarted(view, event.turnId, event.messageId, now);
    case "turnFinished":
      return onTurnFinished(view, event.turn);
    case "assistantStarted":
      return startStreaming(view, event.agentId, event.messageId, now);
    case "textDelta":
      return appendDelta(view, event.agentId, event.messageId, "text", event.text, now);
    case "thinkingDelta":
      return appendDelta(view, event.agentId, event.messageId, "thinking", event.text, now);
    case "assistantFinished":
      return finishAssistant(view, event.agentId, event.message);
    case "toolStarted":
      return startTool(view, event, now);
    case "toolProgress":
      return progressTool(view, event.agentId, event.callId, event.text, now);
    case "toolFinished":
      return finishTool(view, event, now);
    case "approvalRequested":
      return { ...view, approvals: put(view.approvals, event.request.requestId, event.request) };
    case "approvalResolved":
      return { ...view, approvals: drop(view.approvals, event.requestId) };
    case "questionAsked": {
      const { requestId, agentId, questions } = event;
      return { ...view, questions: put(view.questions, requestId, { requestId, agentId, questions }) };
    }
    case "questionResolved":
      return { ...view, questions: drop(view.questions, event.requestId) };
    case "planProposed": {
      const { requestId, agentId, plan } = event;
      return { ...view, plans: put(view.plans, requestId, { requestId, agentId, plan }) };
    }
    case "planResolved":
      return { ...view, plans: drop(view.plans, event.requestId) };
    case "todosUpdated":
      return { ...view, todos: put(view.todos, event.agentId, event.todos) };
    case "agentStarted":
    case "agentUpdated":
      return { ...view, agents: put(view.agents, event.info.agentId, event.info) };
    case "jobUpdated":
      return { ...view, jobs: put(view.jobs, event.job.jobId, event.job) };
    case "usageUpdated": {
      const main = event.agentId === MAIN_AGENT;
      return {
        ...view,
        agentUsage: put(view.agentUsage, event.agentId, event.usage),
        usage: event.sessionUsage,
        costUsd: event.costUsd,
        contextTokens: main ? event.contextTokens : view.contextTokens,
        contextLimit: main ? event.contextLimit : view.contextLimit,
      };
    }
    case "modeChanged":
      return { ...view, mode: event.mode, info: patchInfo(view.info, { mode: event.mode }) };
    case "planningChanged":
      return { ...view, planning: event.planning, info: patchInfo(view.info, { planning: event.planning }) };
    case "modelChanged":
      return { ...view, model: event.model, info: patchInfo(view.info, { model: event.model }) };
    case "effortChanged":
      return { ...view, effort: event.effort, info: patchInfo(view.info, { effort: event.effort }) };
    case "compactionStarted":
      return { ...view, compacting: true };
    case "compacted":
      return { ...view, compactions: [...view.compactions, event.marker], compacting: false };
    case "checkpointCreated":
      return {
        ...view,
        checkpoints: upsertBy(view.checkpoints, event.checkpoint, (c) => c.checkpointId),
      };
    case "checkRecorded":
      return { ...view, checks: upsertBy(view.checks, event.record, (r) => r.recordId) };
    case "verificationChanged":
      return { ...view, verification: event.outcome };
    case "completionClaimUnchecked":
      return view.activeTurn ? { ...view, claims: put(view.claims, view.activeTurn.turnId, event.claim) } : view;
    case "hookRan": {
      const { hookEvent, command, blocked, message } = event;
      return addHookRun(view, { hookEvent, command, blocked, message }, now);
    }
    case "commandOutput":
      return addCommandOutput(view, event.name, event.markdown, now);
    case "contextReport":
      return { ...view, contextBreakdown: event.breakdown };
    case "notice":
      return addNotice(view, event.level, event.text, now);
    case "retrying":
      return {
        ...view,
        retrying: { attempt: event.attempt, delayMs: event.delayMs, reason: event.reason, at: now },
      };
    case "titleChanged":
      return { ...view, title: event.title, info: patchInfo(view.info, { title: event.title }) };
    case "queueChanged":
      return { ...view, queue: event.queued };
    case "trustRequired":
      return { ...view, trustRequest: { projectRoot: event.projectRoot, defines: event.defines } };
    case "routeChosen":
      return addRoute(view, event.route, now);
    case "taskViewApplied":
      return { ...view, taskViews: upsertBy(view.taskViews, event.view, (v) => v.boundary) };
    case "suggested":
      return addSuggestion(view, event.suggestion);
    case "suggestionResolved":
      return dropSuggestion(view, event.suggestionId);
    case "turnToneJudged":
      return { ...view, turnTones: put(view.turnTones, event.turnId, event.tone) };
    case "urgencyScored":
      return { ...view, urgency: put(view.urgency, event.urgency.key, event.urgency.urgency) };
    case "workUpdated":
      return { ...view, work: applyWorkUpdate(view.work, event.update) };
    case "taskPacketSent": {
      const { agentId, taskId, tokens, reason } = event;
      return { ...view, packets: addPacket(view.packets, { agentId, taskId, tokens, reason, at: now }) };
    }
    case "error":
      return addError(view, event.message, now);
    default:
      return view;
  }
}
