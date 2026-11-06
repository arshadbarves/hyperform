import type { ToolStatus } from "../../protocol/ToolStatus";
import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import { mapRecord, put } from "./records";
import { PROGRESS_TAIL_CHARS, type SessionView, type ToolCallView } from "./types";

export interface ToolStart {
  agentId: string;
  callId: string;
  tool: string;
  title: string;
  input: JsonValue;
}

export interface ToolFinish {
  agentId: string;
  callId: string;
  status: ToolStatus;
  summary: string;
  output: string;
  durationMs: number;
}

function placeholder(agentId: string, callId: string, now: number): ToolCallView {
  return {
    callId,
    agentId,
    tool: "",
    title: "",
    input: null,
    status: "running",
    summary: "",
    output: "",
    progress: "",
    durationMs: null,
    startedAt: now,
  };
}

/** Keep the last `max` characters, starting at a line boundary when one is near. */
export function capTail(text: string, max = PROGRESS_TAIL_CHARS): string {
  if (text.length <= max) return text;
  const cut = text.slice(text.length - max);
  const newline = cut.indexOf("\n");
  return newline >= 0 && newline < 256 ? cut.slice(newline + 1) : cut;
}

export function startTool(view: SessionView, start: ToolStart, now: number): SessionView {
  const prev = view.tools[start.callId];
  const next: ToolCallView = {
    ...(prev ?? placeholder(start.agentId, start.callId, now)),
    agentId: start.agentId,
    tool: start.tool,
    title: start.title,
    input: start.input,
    status: "running",
    startedAt: prev?.startedAt ?? now,
  };
  return { ...view, tools: put(view.tools, start.callId, next) };
}

export function progressTool(
  view: SessionView,
  agentId: string,
  callId: string,
  text: string,
  now: number,
): SessionView {
  const prev = view.tools[callId] ?? placeholder(agentId, callId, now);
  return { ...view, tools: put(view.tools, callId, { ...prev, progress: capTail(prev.progress + text) }) };
}

export function finishTool(view: SessionView, finish: ToolFinish, now: number): SessionView {
  const prev = view.tools[finish.callId] ?? placeholder(finish.agentId, finish.callId, now);
  const next: ToolCallView = {
    ...prev,
    status: finish.status,
    summary: finish.summary,
    output: finish.output,
    durationMs: finish.durationMs,
  };
  return { ...view, tools: put(view.tools, finish.callId, next) };
}

/** A finished foreground turn leaves no main-agent call running. */
export function settleRunningTools(view: SessionView, agentId: string): SessionView {
  const tools = mapRecord(view.tools, (tool): ToolCallView =>
    tool.agentId === agentId && tool.status === "running" ? { ...tool, status: "cancelled" } : tool,
  );
  return tools === view.tools ? view : { ...view, tools };
}
