/** Protocol builders for unit tests and the dev-only mock backend (lib/runtime/dev); never imported by app code. */
import type { AgentInfo } from "../protocol/AgentInfo";
import type { ApprovalRequest } from "../protocol/ApprovalRequest";
import type { CheckRecord } from "../protocol/CheckRecord";
import type { ContentBlock } from "../protocol/ContentBlock";
import type { Event } from "../protocol/Event";
import type { EventEnvelope } from "../protocol/EventEnvelope";
import type { JobInfo } from "../protocol/JobInfo";
import type { Message } from "../protocol/Message";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionSnapshot } from "../protocol/SessionSnapshot";
import type { TurnRecord } from "../protocol/TurnRecord";
import type { Usage } from "../protocol/Usage";
import type { JsonValue } from "../protocol/serde_json/JsonValue";

export const text = (value: string): ContentBlock => ({ type: "text", text: value });
export const thinking = (value: string): ContentBlock => ({ type: "thinking", text: value, signature: null });
export const toolUse = (id: string, name: string, input: JsonValue = {}): ContentBlock => ({
  type: "toolUse",
  id,
  name,
  input,
});
export const toolResult = (id: string, value: string, isError = false): ContentBlock => ({
  type: "toolResult",
  toolUseId: id,
  content: [{ type: "text", text: value }],
  isError,
});

export function user(id: string, content: string | ContentBlock[], createdAt = 0): Message {
  return { id, role: "user", content: typeof content === "string" ? [text(content)] : content, createdAt };
}

export function assistant(id: string, content: string | ContentBlock[], createdAt = 0): Message {
  return {
    id,
    role: "assistant",
    content: typeof content === "string" ? [text(content)] : content,
    createdAt,
  };
}

export function usage(over: Partial<Usage> = {}): Usage {
  return {
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    reasoningTokens: 0,
    ...over,
  };
}

export function info(over: Partial<SessionInfo> = {}): SessionInfo {
  return {
    sessionId: "S1",
    title: null,
    projectRoot: "/repo",
    model: "anthropic/claude-sonnet-4",
    mode: "default",
    planning: false,
    effort: null,
    createdAt: 1,
    updatedAt: 1,
    legacy: false,
    ...over,
  };
}

export function snapshot(over: Partial<SessionSnapshot> = {}): SessionSnapshot {
  return {
    info: info(),
    status: "idle",
    messages: [],
    compactions: [],
    taskViews: [],
    turns: [],
    todos: [],
    agents: [],
    jobs: [],
    checks: [],
    checkpoints: [],
    pendingApprovals: [],
    pendingQuestions: [],
    pendingPlans: [],
    queued: [],
    usage: usage(),
    costUsd: 0,
    contextTokens: 0,
    contextLimit: 200_000,
    ...over,
  };
}

export function turnRecord(over: Partial<TurnRecord> = {}): TurnRecord {
  return {
    turnId: "t1",
    messageId: "u1",
    outcome: { type: "completed" },
    verification: { status: "notApplicable" },
    usage: usage(),
    costUsd: 0,
    startedAt: 0,
    finishedAt: 100,
    ...over,
  };
}

export function agentInfo(over: Partial<AgentInfo> = {}): AgentInfo {
  return {
    agentId: "agt_1",
    parentId: "main",
    callId: null,
    agentType: "explore",
    description: "Find the bug",
    model: "anthropic/claude-haiku",
    background: false,
    isolation: "shared",
    worktree: null,
    status: "running",
    depth: 1,
    startedAt: 10,
    finishedAt: null,
    usage: usage(),
    costUsd: 0,
    toolCalls: 0,
    resultPreview: null,
    error: null,
    readWeb: false,
    ...over,
  };
}

export function jobInfo(over: Partial<JobInfo> = {}): JobInfo {
  return {
    jobId: "job_1",
    kind: "shell",
    label: "npm run dev",
    owner: "main",
    agentId: null,
    status: "running",
    exitCode: null,
    startedAt: 10,
    finishedAt: null,
    outputTail: "",
    ...over,
  };
}

export function checkRecord(over: Partial<CheckRecord> = {}): CheckRecord {
  return {
    recordId: "rec_1",
    checkId: "test",
    label: "cargo test",
    kind: "test",
    command: "cargo test",
    cwd: "/repo",
    agentId: "main",
    exitCode: 0,
    passed: true,
    timedOut: false,
    startedAt: 50,
    durationMs: 1200,
    tests: { passed: 12, failed: 0, skipped: 1 },
    artifact: null,
    fingerprintBefore: "a",
    fingerprintAfter: "a",
    outputTail: "ok",
    ...over,
  };
}

export function approval(over: Partial<ApprovalRequest> = {}): ApprovalRequest {
  return {
    requestId: "req_1",
    agentId: "main",
    callId: "c1",
    tool: "Bash",
    title: "Run cargo test",
    input: { command: "cargo test" },
    preview: { type: "command", command: "cargo test", description: null },
    reason: "Bash is not allowed by default",
    suggestedRule: "Bash(cargo test:*)",
    canPersist: true,
    ...over,
  };
}

export function envelope(event: Event, seq: number, sessionId = "S1"): EventEnvelope {
  return { sessionId, seq, event };
}
