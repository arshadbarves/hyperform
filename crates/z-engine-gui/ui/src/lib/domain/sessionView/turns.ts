import type { Message } from "../../protocol/Message";
import type { SessionStatus } from "../../protocol/SessionStatus";
import type { TurnRecord } from "../../protocol/TurnRecord";
import { hasToolResults } from "../timeline/blocks";
import { put, upsertBy, upsertMessage } from "./records";
import { dropAgentStreams } from "./streaming";
import { settleRunningTools } from "./tools";
import { MAIN_AGENT, type SessionView } from "./types";

export function withStatus(view: SessionView, status: SessionStatus): SessionView {
  if (status !== "idle") return { ...view, status };
  return { ...view, status, activeTurn: null, retrying: null, compacting: false };
}

export function onUserMessage(
  view: SessionView,
  message: Message,
  turnId: string | null,
  steering: boolean,
): SessionView {
  const messages = upsertMessage(view.messages, message);
  if (steering) {
    return { ...view, messages, steeringIds: put(view.steeringIds, message.id, true) };
  }
  const knownTurn = turnId !== null && Object.values(view.turnStarts).includes(turnId);
  if (turnId !== null && !knownTurn && !hasToolResults(message)) {
    return { ...view, messages, turnStarts: put(view.turnStarts, message.id, turnId) };
  }
  return { ...view, messages };
}

export function onTurnStarted(
  view: SessionView,
  turnId: string,
  messageId: string,
  now: number,
): SessionView {
  return {
    ...view,
    activeTurn: { turnId, messageId, startedAt: now, costAtStart: view.costUsd },
    turnStarts: put(view.turnStarts, messageId, turnId),
    verification: null,
  };
}

export function onTurnFinished(view: SessionView, turn: TurnRecord): SessionView {
  const settled = dropAgentStreams(settleRunningTools(view, MAIN_AGENT), MAIN_AGENT);
  return {
    ...settled,
    turns: upsertBy(view.turns, turn, (t) => t.turnId),
    turnStarts: put(view.turnStarts, turn.messageId, turn.turnId),
    activeTurn: view.activeTurn?.turnId === turn.turnId ? null : view.activeTurn,
    verification: null,
    retrying: null,
    compacting: false,
  };
}
