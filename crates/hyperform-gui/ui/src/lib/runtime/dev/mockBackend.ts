import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { credentialKey } from "../../domain/settings/credentials";
import { assistant, user } from "../../domain/testFixtures";
import { PLAN_MOVED_NOTICE } from "../../domain/work/trace";
import type { Command } from "../../protocol/Command";
import type { Event } from "../../protocol/Event";
import type { EventEnvelope } from "../../protocol/EventEnvelope";
import type { WorkEdit } from "../../protocol/WorkEdit";
import type { WorkSnapshot } from "../../protocol/WorkSnapshot";
import type { WorkUpdate } from "../../protocol/WorkUpdate";
import { editPlan, releaseBound, type MockEdit } from "./mockEdits";
import { settleHelper, type Decision } from "./mockHelpers";
import { MOCK_FEATURES, MOCK_SETTINGS } from "./mockSettings";
import { MockWorld, type MockChat } from "./mockWorld";

/**
 * A pretend engine for browser QA of the work-plan screens: `npm run dev`
 * in a plain browser (main.ts installs it in dev builds outside Tauri).
 * It answers the GUI's IPC commands from MockWorld, applies plan edits
 * roughly as the kernel does, and emits `engineEvent`s the way the engine
 * does (a snapshot on open, attach and detach; `workUpdated` per edit;
 * `agentUpdated` on Apply and Discard). Nothing persists: a reload starts
 * over.
 */

type Args = Record<string, unknown>;

/** How long a paused plan's helpers take to reach their next step. */
const NEXT_STEP_MS = 4000;
const LISTS = new Set([
  "fetch_model_catalog",
  "list_commands",
  "list_agents",
  "list_files",
  "list_changed_files",
  "session_changed_files",
  "list_instruction_files",
  "agent_transcript",
  "work_why",
]);

const helperIds = (work: WorkSnapshot) =>
  work.tasks.flatMap((t) => (t.owner && t.owner.agentId !== "main" ? [t.owner.agentId] : []));

class MockBackend {
  readonly world: MockWorld;
  #seq = 0;
  #said = 0;

  constructor(world: MockWorld) {
    this.world = world;
  }

  handle(cmd: string, args: Args): unknown {
    const str = (key: string) => args[key] as string;
    if (LISTS.has(cmd)) return [];
    switch (cmd) {
      case "app_info":
        return { version: "0.0.0-mock", configDir: "/mock/config", dataDir: "/mock/data", nativeGlass: false };
      case "list_workspaces":
        return this.world.workspaces();
      case "add_workspace":
        return str("path");
      case "list_sessions":
        return this.world.sessions();
      case "open_session":
        return this.#open(str("projectRoot"), (args.sessionId as string | null) ?? null);
      case "delete_session":
        this.world.chats.delete(str("sessionId"));
        return null;
      case "send_command":
        return this.#command(this.#chat(str("sessionId")), args.command as Command);
      case "work_plans":
        return this.world.planList(str("projectRoot"));
      case "work_parallel":
        return this.world.parallel(str("sessionId"));
      case "work_packet":
        throw new Error("The mock backend has no task packets");
      case "get_settings":
        return { settings: MOCK_SETTINGS, layers: [], warnings: [] };
      case "get_layer":
        return { path: "/mock/config/settings.toml", exists: false, raw: {} };
      case "feature_catalog":
        return MOCK_FEATURES;
      case "credential_status":
        return { [credentialKey(MOCK_SETTINGS.provider.base_url)]: { hasKey: true, hint: "sk-…mock" } };
      case "trust_status":
        return { trusted: true, projectDefines: [] };
      case "check_for_update":
        return { available: false, current: "0.0.0-mock" };
      case "list_extensions":
        return { agents: [], commands: [], skills: [], rules: [], outputStyles: [], errors: [] };
      case "reveal_path":
      case "open_path":
        console.info(`[mock backend] ${cmd}`, args.path);
        return null;
      default:
        return null;
    }
  }

  #chat(id: string): MockChat {
    const chat = this.world.chats.get(id);
    if (!chat) throw new Error(`no chat ${id}`);
    return chat;
  }

  #send(sessionId: string, event: Event) {
    const envelope: EventEnvelope = { sessionId, seq: ++this.#seq, event };
    void emit("engineEvent", envelope);
  }

  #snapshot(chat: MockChat) {
    this.#send(chat.info.sessionId, { type: "snapshot", snapshot: this.world.snapshotOf(chat) });
  }

  #open(projectRoot: string, sessionId: string | null): string {
    const chat = sessionId ? this.#chat(sessionId) : this.world.newChat(projectRoot);
    setTimeout(() => this.#snapshot(chat), 0);
    return chat.info.sessionId;
  }

  #command(chat: MockChat, command: Command): null {
    if (command.type === "editWork") this.#edit(command.planId, command.edit);
    else if (command.type === "attachPlan") this.#attach(chat, command.planId);
    else if (command.type === "detachPlan") {
      chat.planId = null;
      this.#snapshot(chat);
    } else if (command.type === "submit") this.#submit(chat, command.text);
    else if (command.type === "applyAgentChanges") this.#settle(chat, command.agentId, "applied");
    else if (command.type === "discardAgentChanges") this.#settle(chat, command.agentId, "discarded");
    return null;
  }

  /** Apply or Discard from the Agents tab: the helper's worktree, then its plan task. */
  #settle(chat: MockChat, agentId: string, decision: Decision) {
    const agent = chat.agents.find((a) => a.agentId === agentId);
    if (!agent) return;
    const plan = chat.planId ? this.world.plans.get(chat.planId) : undefined;
    const settled = settleHelper(agent, plan?.work ?? null, decision);
    if (typeof settled === "string") {
      this.#send(chat.info.sessionId, { type: "notice", level: "warn", text: settled });
      return;
    }
    chat.agents = chat.agents.map((a) => (a.agentId === agentId ? settled.info : a));
    this.#send(chat.info.sessionId, { type: "agentUpdated", info: settled.info });
    if (!plan || !settled.tasks.size) return;
    const result = editPlan(plan.work, null, Date.now(), settled.tasks);
    if (typeof result !== "string") this.#commit(plan.work.plan.planId, result);
  }

  #edit(planId: string, edit: WorkEdit) {
    const plan = this.world.plans.get(planId);
    if (!plan) throw new Error(`no plan ${planId}`);
    const helpers = helperIds(plan.work);
    const result = editPlan(plan.work, edit, Date.now());
    if (typeof result === "string") throw new Error(result);
    this.#commit(planId, result);
    if (edit.type === "stop") this.#cancelHelpers(planId, helpers);
    if (edit.type === "pause" && helpers.length) setTimeout(() => this.#nextStep(planId), NEXT_STEP_MS);
  }

  /** A paused plan's helpers reach their next step: their tasks go back to Pending, resumable. */
  #nextStep(planId: string) {
    const plan = this.world.plans.get(planId);
    if (!plan || plan.work.plan.status !== "paused") return;
    const helpers = helperIds(plan.work);
    const result = editPlan(plan.work, null, Date.now(), releaseBound(plan.work, "Paused at its next step"));
    if (typeof result === "string" || !result.tasks.length) return;
    this.#commit(planId, result);
    this.#cancelHelpers(planId, helpers);
  }

  #commit(planId: string, { work, tasks }: MockEdit) {
    const plan = this.world.plans.get(planId);
    if (!plan) return;
    plan.work = work;
    const update: WorkUpdate = {
      planId,
      revision: work.plan.revision,
      plan: work.plan,
      tasks,
      entries: [],
      change: null,
      audit: work.audit,
      findings: work.findings,
      health: work.health,
      replanHeld: work.replanHeld,
    };
    for (const chat of this.world.chats.values()) {
      if (chat.planId === planId) this.#send(chat.info.sessionId, { type: "workUpdated", update });
    }
  }

  #cancelHelpers(planId: string, agentIds: string[]) {
    const now = Date.now();
    for (const chat of this.world.chats.values()) {
      if (chat.planId !== planId) continue;
      chat.agents = chat.agents.map((a) => {
        if (!agentIds.includes(a.agentId) || (a.status !== "running" && a.status !== "waiting")) return a;
        const info = { ...a, status: "cancelled" as const, finishedAt: now };
        this.#send(chat.info.sessionId, { type: "agentUpdated", info });
        return info;
      });
    }
  }

  /** Attach moves the plan: the chat that had it gets a snapshot without it and a notice. */
  #attach(chat: MockChat, planId: string) {
    const plan = this.world.plans.get(planId);
    if (!plan) throw new Error(`no plan ${planId}`);
    for (const other of this.world.chats.values()) {
      if (other === chat || other.planId !== planId) continue;
      other.planId = null;
      this.#snapshot(other);
      this.#send(other.info.sessionId, { type: "notice", level: "info", text: PLAN_MOVED_NOTICE });
    }
    chat.planId = planId;
    plan.lastSessionId = chat.info.sessionId;
    this.#snapshot(chat);
  }

  #submit(chat: MockChat, text: string) {
    const now = Date.now();
    const n = ++this.#said;
    const reply = "Mock backend: no model runs here. Use the Plan tab to try the plan controls.";
    chat.messages = [...chat.messages, user(`${chat.info.sessionId}-u${n}`, text, now), assistant(`${chat.info.sessionId}-a${n}`, reply, now + 1)];
    chat.info = { ...chat.info, title: chat.info.title ?? text.slice(0, 48), updatedAt: now };
    this.#snapshot(chat);
  }
}

/** Answer every IPC call from the mock world; call before anything invokes the engine. */
export function installMockBackend(): void {
  const backend = new MockBackend(new MockWorld());
  mockWindows("main");
  mockIPC((cmd, args) => backend.handle(cmd, (args ?? {}) as Args), { shouldMockEvents: true });
  console.info("[mock backend] installed: work-plan fixtures in /Users/you/notes-app (see lib/runtime/dev/mockBackend.ts)");
}
