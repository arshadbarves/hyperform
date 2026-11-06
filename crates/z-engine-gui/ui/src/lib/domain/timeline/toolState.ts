import type { AgentInfo } from "../../protocol/AgentInfo";
import type { MediaSource } from "../../protocol/MediaSource";
import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import { MAIN_AGENT, type StreamingMessage, type ToolCallStatus, type ToolCallView } from "../sessionView/types";
import { str } from "../tools/toolInput";
import type { ToolResultInfo, ToolUseRef } from "./blocks";

/** Everything a tool card renders, merged from the tool_use block, its result and live events. */
export interface ToolCallState {
  callId: string;
  name: string;
  input: JsonValue;
  title: string;
  status: ToolCallStatus;
  summary: string;
  output: string;
  progress: string;
  durationMs: number | null;
  startedAt: number | null;
  images: MediaSource[];
}

export function resolveToolCall(
  use: ToolUseRef,
  result: ToolResultInfo | undefined,
  live: ToolCallView | undefined,
  sessionBusy: boolean,
): ToolCallState {
  const fromResult: ToolCallStatus | null = result ? (result.isError ? "error" : "ok") : null;
  // A persisted result outranks a live "running" that missed its toolFinished.
  const liveStatus = live && !(live.status === "running" && fromResult) ? live.status : null;
  const status: ToolCallStatus = liveStatus ?? fromResult ?? (sessionBusy ? "running" : "cancelled");
  return {
    callId: use.callId,
    name: use.name || live?.tool || "",
    input: use.input ?? live?.input ?? null,
    title: live?.title ?? "",
    status,
    summary: live?.summary ?? "",
    output: live?.output || result?.text || "",
    progress: live?.progress ?? "",
    durationMs: live?.durationMs ?? null,
    startedAt: live?.startedAt ?? null,
    images: result?.images ?? [],
  };
}

/** Main-agent streams in arrival order (normally zero or one). */
export function liveStreams(
  streaming: Record<string, StreamingMessage>,
  agentId = MAIN_AGENT,
): StreamingMessage[] {
  return Object.values(streaming)
    .filter((s) => s.agentId === agentId)
    .sort((a, b) => a.startedAt - b.startedAt);
}

const AGENT_TOOLS = new Set(["Agent", "Task"]);

export function isAgentTool(name: string): boolean {
  return AGENT_TOOLS.has(name);
}

/**
 * Link `Agent` tool calls to the agent runs they spawned: by `callId` when the
 * engine recorded it, otherwise by the caller's description and type (in
 * start order), then pair any leftovers positionally.
 */
export function linkAgentCalls(
  uses: ToolUseRef[],
  agents: AgentInfo[],
  parentId = MAIN_AGENT,
): Record<string, string> {
  const calls = uses.filter((u) => isAgentTool(u.name));
  const pool = agents
    .filter((a) => a.parentId === parentId || (parentId === MAIN_AGENT && a.parentId === null))
    .sort((a, b) => a.startedAt - b.startedAt);
  const taken = new Set<string>();
  const links: Record<string, string> = {};
  const typeOf = (u: ToolUseRef) => str(u.input, "subagent_type", "agent_type");
  const claim = (use: ToolUseRef, match: (a: AgentInfo) => boolean) => {
    const agent = pool.find((a) => !taken.has(a.agentId) && match(a));
    if (!agent) return;
    taken.add(agent.agentId);
    links[use.callId] = agent.agentId;
  };
  for (const use of calls) {
    claim(use, (a) => a.callId === use.callId);
  }
  for (const use of calls) {
    if (links[use.callId]) continue;
    const description = str(use.input, "description");
    const type = typeOf(use);
    claim(use, (a) => a.description === description && (!type || a.agentType === type));
  }
  for (const use of calls) {
    if (links[use.callId]) continue;
    const type = typeOf(use);
    claim(use, (a) => !type || a.agentType === type);
  }
  return links;
}
