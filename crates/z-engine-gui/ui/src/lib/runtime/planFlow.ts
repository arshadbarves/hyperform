import { workspaceStore } from "../workspaces";
import { attachPlan, editWork, ensureSession, newChat, resolvePlan, send } from "./actions";
import { sessions } from "./sessions.svelte";

export type BuildTarget = "here" | "newChat";
/** The first message of a chat that builds an attached plan. */
export const BUILD_PROMPT = "Build the attached plan.";

export interface BuildRequest {
  planId: string;
  projectRoot: string;
  target: BuildTarget;
  /** The agent's plan waiting for review, answered by this approval. */
  requestId: string | null;
  editedPlan?: string | null;
  /** Also approve parallel helpers for the plan (`work_parallel`), right after the approval. */
  parallel?: boolean;
}

/** An idle chat is told `prompt`; a busy one carries on with the plan. A new chat may start in plan mode. */
async function startWith(sessionId: string, prompt: string): Promise<boolean> {
  const view = sessions.view(sessionId);
  if ((view?.planning ?? true) && !(await send({ type: "setPlanning", planning: false }, sessionId))) return false;
  if (view && view.status !== "idle") return true;
  return send({ type: "submit", text: prompt, attachments: [] }, sessionId);
}

/**
 * A new chat in the plan's project takes the plan (moving it from the
 * chat that had it) and, given a prompt, leaves plan mode and is told to
 * go on. Returns the new chat, or null when a step failed.
 */
export async function takePlanInNewChat(planId: string, projectRoot: string, prompt: string | null): Promise<string | null> {
  workspaceStore.setActive(projectRoot);
  const id = await newChat();
  if (!id || !(await attachPlan(planId, id))) return null;
  if (prompt === null) return id;
  return (await startWith(id, prompt)) ? id : null;
}

/** Attach a plan to this chat (a new one when none is open), moving it from the chat that had it. */
export async function attachPlanHere(planId: string): Promise<string | null> {
  const id = await ensureSession();
  return id && (await attachPlan(planId, id)) ? id : null;
}

/**
 * Approve a draft (or the changes after replanning) and build it. "here"
 * builds in this chat; "newChat" stops this chat, then a new chat in the
 * plan's project attaches the plan and starts from its packet. With
 * `parallel`, parallel helpers are approved right after the plan. Returns
 * the chat that builds, or null when a step failed.
 */
export async function buildPlan(req: BuildRequest): Promise<string | null> {
  const from = sessions.activeId;
  if (!from) return null;
  const handoff = req.target === "newChat";
  const approved = req.requestId
    ? await resolvePlan(req.requestId, { type: "approve", mode: null, editedPlan: req.editedPlan ?? null, handoff })
    : await editWork(req.planId, { type: "approve" }, from);
  if (!approved) return null;
  if (req.parallel && !(await editWork(req.planId, { type: "approveParallel" }, from))) return null;
  if (!handoff) return req.requestId || (await startWith(from, BUILD_PROMPT)) ? from : null;
  return takePlanInNewChat(req.planId, req.projectRoot, BUILD_PROMPT);
}

/** Keep planning: answer the agent's plan with feedback, or ask again in plan mode. */
export async function keepPlanning(requestId: string | null, feedback: string): Promise<boolean> {
  if (requestId) return resolvePlan(requestId, { type: "revise", feedback });
  const id = sessions.activeId;
  if (!id || !feedback) return false;
  if (!sessions.view(id)?.planning && !(await send({ type: "setPlanning", planning: true }, id))) return false;
  return send({ type: "submit", text: feedback, attachments: [] }, id);
}
