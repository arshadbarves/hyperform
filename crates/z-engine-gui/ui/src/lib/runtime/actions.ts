import {
  deleteSession as ipcDeleteSession,
  exportSession,
  openSession as ipcOpenSession,
  sendCommand,
  type ExportFormat,
} from "../commands";
import type { ApprovalDecision } from "../protocol/ApprovalDecision";
import type { Attachment } from "../protocol/Attachment";
import type { Command } from "../protocol/Command";
import type { Effort } from "../protocol/Effort";
import type { PermissionMode } from "../protocol/PermissionMode";
import type { PlanDecision } from "../protocol/PlanDecision";
import type { QuestionAnswer } from "../protocol/QuestionAnswer";
import type { RewindScope } from "../protocol/RewindScope";
import type { WorkEdit } from "../protocol/WorkEdit";
import { startShell } from "../shellStore";
import { workspaceStore } from "../workspaces";
import { catalogs } from "./catalogs.svelte";
import { sessionList } from "./sessionList.svelte";
import { sessions } from "./sessions.svelte";
import { errorText, pushToast } from "./toasts";

const opening = new Map<string, Promise<void>>();
let creating: Promise<string | null> | null = null;

function workspaceRoot(): string | null {
  const { active, roots } = workspaceStore.getSnapshot();
  return active ?? roots[0] ?? null;
}

/** Project root of the active session, else the active workspace. */
export function activeProjectRoot(): string | null {
  return sessions.active?.info?.projectRoot ?? workspaceRoot();
}

/** Show a session; only sessions not yet live this run need `open_session`. */
export async function openSession(sessionId: string, projectRoot: string): Promise<void> {
  sessions.activate(sessionId);
  if (projectRoot) workspaceStore.setActive(projectRoot);
  void catalogs.ensure(projectRoot);
  if (sessions.view(sessionId)?.info || opening.has(sessionId)) return;
  const task = ipcOpenSession(projectRoot, sessionId)
    .then(() => undefined)
    .catch((e: unknown) => {
      pushToast(`Could not open chat · ${errorText(e)}`, "warn");
      if (sessions.activeId === sessionId && !sessions.view(sessionId)?.info) sessions.activate(null);
    })
    .finally(() => opening.delete(sessionId));
  opening.set(sessionId, task);
  await task;
}

/** Create a session in the active workspace; null when no workspace exists. */
export async function newChat(): Promise<string | null> {
  const root = workspaceRoot();
  if (!root) {
    pushToast("Add a workspace folder to start a chat", "info");
    return null;
  }
  try {
    const id = await ipcOpenSession(root, null);
    sessions.activate(id);
    void catalogs.ensure(root);
    return id;
  } catch (e) {
    pushToast(`Could not start a chat · ${errorText(e)}`, "warn");
    return null;
  }
}

/** The active session, creating one on first use. */
export async function ensureSession(): Promise<string | null> {
  if (sessions.activeId) return sessions.activeId;
  creating ??= newChat().finally(() => {
    creating = null;
  });
  return creating;
}

export async function send(command: Command, sessionId = sessions.activeId): Promise<boolean> {
  if (!sessionId) return false;
  await opening.get(sessionId);
  try {
    await sendCommand(sessionId, command);
    return true;
  } catch (e) {
    pushToast(errorText(e), "warn");
    return false;
  }
}

async function sendEnsured(command: Command): Promise<boolean> {
  const id = await ensureSession();
  return id ? send(command, id) : false;
}

export const submitPrompt = (text: string, attachments: Attachment[]) =>
  sendEnsured({ type: "submit", text, attachments });
export const steer = (text: string) => send({ type: "steer", text });
export const interrupt = (text: string | null) => send({ type: "interrupt", text });
export const cancelTurn = () => send({ type: "cancel" });
export const editQueue = (queued: string[]) => send({ type: "editQueue", queued });
export const setMode = (mode: PermissionMode) => sendEnsured({ type: "setMode", mode });
export const setPlanning = (planning: boolean) => sendEnsured({ type: "setPlanning", planning });
export const setModel = (model: string) => sendEnsured({ type: "setModel", model });
export const setEffort = (effort: Effort | null) => sendEnsured({ type: "setEffort", effort });
export const compact = (instructions: string | null = null) => send({ type: "compact", instructions });
export const includeFullHistory = () => send({ type: "includeFullHistory" });
export const rewind = (messageId: string, scope: RewindScope) => send({ type: "rewind", messageId, scope });
export const killJob = (jobId: string) => send({ type: "killJob", jobId });
export const applyAgentChanges = (agentId: string) => send({ type: "applyAgentChanges", agentId });
export const discardAgentChanges = (agentId: string) => send({ type: "discardAgentChanges", agentId });
export const runCommand = (name: string, args = "") => sendEnsured({ type: "runCommand", name, args });

export const resolveApproval = (requestId: string, decision: ApprovalDecision, sessionId?: string) =>
  send({ type: "resolveApproval", requestId, decision }, sessionId ?? sessions.activeId);
export const answerQuestion = (requestId: string, answers: QuestionAnswer[] | null) =>
  send({ type: "answerQuestion", requestId, answers });
export const resolvePlan = (requestId: string, decision: PlanDecision) =>
  send({ type: "resolvePlan", requestId, decision });
/** Acted on (`accepted`) or dismissed a decision-use card; the engine traces it and clears the card. */
export const resolveSuggestion = (suggestionId: string, accepted: boolean) =>
  send({ type: "resolveSuggestion", suggestionId, accepted });

/** A user edit of a work plan; the engine records it with you as the actor. */
export const editWork = (planId: string, edit: WorkEdit, sessionId = sessions.activeId) =>
  send({ type: "editWork", planId, edit }, sessionId);
export const attachPlan = (planId: string, sessionId = sessions.activeId) => send({ type: "attachPlan", planId }, sessionId);
export const detachPlan = () => send({ type: "detachPlan" });

/** Answer `trustRequired`; trusting reloads the session (its snapshot clears the banner). */
export async function answerTrust(trusted: boolean, sessionId = sessions.activeId): Promise<boolean> {
  if (!sessionId) return false;
  const ok = await send({ type: "trustWorkspace", trusted }, sessionId);
  if (ok) sessions.dismissTrust(sessionId);
  return ok;
}

/** `!cmd`: echo into the terminal drawer, then let the engine run it. */
export async function runShell(command: string): Promise<boolean> {
  startShell(command);
  return sendEnsured({ type: "shell", command });
}

export async function reloadExtensions(): Promise<void> {
  if (await send({ type: "reloadExtensions" })) await catalogs.reload(activeProjectRoot());
}

export async function exportTranscript(format: ExportFormat): Promise<void> {
  const id = sessions.activeId;
  if (!id) {
    pushToast("Open a chat to export it", "info");
    return;
  }
  try {
    const text = await exportSession(id, format);
    await navigator.clipboard.writeText(text);
    pushToast(`Transcript copied · ${format === "json" ? "JSON" : "Markdown"}`, "ok");
  } catch (e) {
    pushToast(`Export failed · ${errorText(e)}`, "warn");
  }
}

export async function deleteChat(sessionId: string): Promise<boolean> {
  try {
    await ipcDeleteSession(sessionId);
  } catch (e) {
    pushToast(`Delete failed · ${errorText(e)}`, "warn");
    return false;
  }
  sessionList.remove(sessionId);
  sessions.forget(sessionId);
  void sessionList.refresh();
  return true;
}
