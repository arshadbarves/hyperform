import type { Message } from "../../protocol/Message";
import { filterRecord, put, upsertMessage } from "./records";
import { MAIN_AGENT, type SessionView, type StreamingMessage } from "./types";

function entry(agentId: string, messageId: string, now: number): StreamingMessage {
  return { messageId, agentId, text: "", thinking: "", startedAt: now };
}

/** The main agent streams only once a compaction is over, including one that failed with just a notice. */
function stillCompacting(view: SessionView, agentId: string): boolean {
  return view.compacting && agentId !== MAIN_AGENT;
}

export function startStreaming(
  view: SessionView,
  agentId: string,
  messageId: string,
  now: number,
): SessionView {
  const prev = view.streaming[messageId];
  const next = prev ?? entry(agentId, messageId, now);
  return {
    ...view,
    streaming: put(view.streaming, messageId, next),
    retrying: null,
    compacting: stillCompacting(view, agentId),
  };
}

export function appendDelta(
  view: SessionView,
  agentId: string,
  messageId: string,
  field: "text" | "thinking",
  delta: string,
  now: number,
): SessionView {
  const prev = view.streaming[messageId] ?? entry(agentId, messageId, now);
  const next: StreamingMessage = { ...prev, [field]: prev[field] + delta };
  return {
    ...view,
    streaming: put(view.streaming, messageId, next),
    retrying: null,
    compacting: stillCompacting(view, agentId),
  };
}

/** The authoritative message replaces the live entry; only main-agent messages join the transcript. */
export function finishAssistant(view: SessionView, agentId: string, message: Message): SessionView {
  const streaming = filterRecord(
    view.streaming,
    (s) => s.messageId !== message.id && s.agentId !== agentId,
  );
  if (agentId !== MAIN_AGENT) return { ...view, streaming };
  return { ...view, streaming, messages: upsertMessage(view.messages, message), retrying: null };
}

export function dropAgentStreams(view: SessionView, agentId: string): SessionView {
  const streaming = filterRecord(view.streaming, (s) => s.agentId !== agentId);
  return streaming === view.streaming ? view : { ...view, streaming };
}
