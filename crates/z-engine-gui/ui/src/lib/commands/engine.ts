import { invoke } from "@tauri-apps/api/core";
import type { Command } from "../protocol/Command";
import type { ContextBreakdown } from "../protocol/ContextBreakdown";
import type { Message } from "../protocol/Message";
import type { SessionSummary } from "../protocol/SessionSummary";
import type { FeatureId } from "../protocol/config/FeatureId";
import type { FeatureMode } from "../protocol/config/FeatureMode";

export type SlashCommandSource = "builtin" | "user" | "project" | "mcp";
export type SlashCommandKind = "engine" | "prompt" | "ui";

export interface SlashCommandInfo {
  name: string;
  description: string;
  argumentHint: string | null;
  source: SlashCommandSource;
  kind: SlashCommandKind;
}

export interface AgentCard {
  name: string;
  description: string;
  source: string;
  model: string | null;
  color: string | null;
}

export type ExportFormat = "markdown" | "json";

export type SessionFileKind = "added" | "modified" | "deleted";

export interface SessionChangedFile {
  path: string;
  kind: SessionFileKind;
}

/** One decision a feature asked about; holds fingerprints and labels, never source text. */
export interface DecisionRecord {
  seq: number;
  atMs: number;
  /** The feature id that asked, e.g. `decisions_compaction`. */
  feature: string;
  question: string;
  /** Recorded in shadow mode: what would have happened. */
  shadow: boolean;
  provider: string;
  inputFingerprint: string;
  /** The model's proposal, even when it was not acted on. */
  answer: string | null;
  confidence: number | null;
  latencyMs: number;
  cached: boolean;
  /** Why the answer was not usable (`timeout`, `low confidence`, `rules`, ...). */
  fallback: string | null;
  /** What the engine finally did (`unchanged`, `kept`, `cleared`, ...). */
  outcome: string;
  overrideReason: string | null;
  tokensSaved: number | null;
}

export interface DecisionSummary {
  count: number;
  shadow: number;
  fallbacks: number;
  p50Ms: number | null;
  p95Ms: number | null;
  tokensSaved: number;
  wouldSaveTokens: number;
}

export interface SessionDecisions {
  features: { id: FeatureId; title: string; mode: FeatureMode }[];
  /** `hybrid` when a decision model is connected, else `rules`. */
  provider: string;
  summary: DecisionSummary;
  /** Newest first. */
  records: DecisionRecord[];
}

/** Opens (or creates, when `sessionId` is null) a session; the engine then emits its snapshot. */
export const openSession = (projectRoot: string, sessionId: string | null) =>
  invoke<string>("open_session", { projectRoot, sessionId });

export const sendCommand = (sessionId: string, command: Command) =>
  invoke<void>("send_command", { sessionId, command });

export const listSessions = () => invoke<SessionSummary[]>("list_sessions");

export const deleteSession = (sessionId: string) =>
  invoke<void>("delete_session", { sessionId });

export const agentTranscript = (sessionId: string, agentId: string) =>
  invoke<Message[]>("agent_transcript", { sessionId, agentId });

export const exportSession = (sessionId: string, format: ExportFormat) =>
  invoke<string>("export_session", { sessionId, format });

/** Last model request JSON, or null when no request was sent yet. */
export const inspectRequest = (sessionId: string) =>
  invoke<unknown>("inspect_request", { sessionId });

/** Token estimate per prompt layer of the next request, or null when the chat is not open. */
export const contextBreakdown = (sessionId: string) =>
  invoke<ContextBreakdown | null>("context_breakdown", { sessionId });

/** Running decision features and the newest decisions, or null when the chat is not open. */
export const sessionDecisions = (sessionId: string, limit: number) =>
  invoke<SessionDecisions | null>("session_decisions", { sessionId, limit });

export const listCommands = (projectRoot: string) =>
  invoke<SlashCommandInfo[]>("list_commands", { projectRoot });

export const listFiles = (projectRoot: string, query: string, limit: number) =>
  invoke<string[]>("list_files", { projectRoot, query, limit });

export const listAgents = (projectRoot: string) =>
  invoke<AgentCard[]>("list_agents", { projectRoot });

export const sessionChangedFiles = (sessionId: string) =>
  invoke<SessionChangedFile[]>("session_changed_files", { sessionId });

export const sessionDiffForFile = (sessionId: string, path: string) =>
  invoke<string>("session_diff_for_file", { sessionId, path });
