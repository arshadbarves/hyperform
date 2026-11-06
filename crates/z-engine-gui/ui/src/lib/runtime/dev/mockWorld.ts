import { fromWorkSnapshot } from "../../domain/sessionView/work";
import { assistant, info, snapshot, user } from "../../domain/testFixtures";
import { needsYouCount, needsYouItems } from "../../domain/work/needsYou";
import type { AgentInfo } from "../../protocol/AgentInfo";
import type { Message } from "../../protocol/Message";
import type { ParallelSet } from "../../protocol/ParallelSet";
import type { PlanSummary } from "../../protocol/PlanSummary";
import type { SessionInfo } from "../../protocol/SessionInfo";
import type { SessionSnapshot } from "../../protocol/SessionSnapshot";
import type { SessionSummary } from "../../protocol/SessionSummary";
import { mockPlans, PROJECT, SITE, type MockPlan } from "./mockPlans";

/**
 * The mock backend's state: chats (with their messages, helpers and
 * attached plan) and plans, and the engine queries derived from them.
 */

export interface MockChat {
  info: SessionInfo;
  messages: Message[];
  agents: AgentInfo[];
  planId: string | null;
}

const CHATS: [id: string, title: string, root: string, planId: string | null, minutesAgo: number, ask: string][] = [
  ["c-build", "Search index", PROJECT, "P-build", 2, "Build the search index plan with helpers."],
  ["c-replan", "Billing API", PROJECT, "P-replan", 15, "Move payments to the new billing API."],
  ["c-paused", "Settings migration", PROJECT, "P-paused", 30, "Migrate settings to the new schema."],
  ["c-draft", "Plan offline sync", PROJECT, "P-draft", 50, "Plan offline sync for the notes app."],
  ["c-login", "Fix the login redirect", PROJECT, null, 70, "The login page redirects twice. Why?"],
  ["c-guide", "User guide refresh", PROJECT, null, 180, "Refresh the user guide, then detach the plan."],
  ["c-done", "Dark mode", PROJECT, "P-done", 2880, "Add dark mode to the editor."],
  ["c-site", "Landing page copy", SITE, null, 1440, "Tighten the landing page copy."],
];

export class MockWorld {
  readonly chats = new Map<string, MockChat>();
  readonly plans = new Map<string, MockPlan>();
  #made = 0;

  constructor(now = Date.now()) {
    const { plans, agents } = mockPlans(now);
    for (const plan of plans) this.plans.set(plan.work.plan.planId, plan);
    for (const [id, title, root, planId, ago, ask] of CHATS) {
      const at = now - ago * 60_000;
      this.chats.set(id, {
        info: info({ sessionId: id, title, projectRoot: root, createdAt: at - 600_000, updatedAt: at }),
        messages: [user(`${id}-u`, ask, at - 600_000), assistant(`${id}-a`, "On it. The Plan tab shows where things stand.", at - 590_000)],
        agents: agents[id] ?? [],
        planId,
      });
    }
  }

  workspaces(): string[] {
    return [PROJECT, SITE];
  }

  sessions(): SessionSummary[] {
    return [...this.chats.values()].map(({ info: i, messages }) => ({
      sessionId: i.sessionId,
      title: i.title,
      projectRoot: i.projectRoot,
      createdAt: i.createdAt,
      updatedAt: i.updatedAt,
      messageCount: messages.length,
      costUsd: 0,
      lastOutcome: messages.length ? { type: "completed" } : null,
      legacy: false,
    }));
  }

  newChat(projectRoot: string, now = Date.now()): MockChat {
    const id = `c-new-${++this.#made}`;
    const chat: MockChat = { info: info({ sessionId: id, projectRoot, createdAt: now, updatedAt: now }), messages: [], agents: [], planId: null };
    this.chats.set(id, chat);
    return chat;
  }

  snapshotOf(chat: MockChat): SessionSnapshot {
    const plan = chat.planId ? this.plans.get(chat.planId) : undefined;
    return snapshot({ info: chat.info, messages: chat.messages, agents: chat.agents, ...(plan ? { work: plan.work } : {}) });
  }

  /** `work_plans`: the project's plans, newest first. */
  planList(projectRoot: string): PlanSummary[] {
    return [...this.plans.values()]
      .filter((p) => p.work.plan.projectRoot === projectRoot)
      .map((p) => this.summary(p))
      .sort((a, b) => b.updatedAt - a.updatedAt);
  }

  summary({ work, lastSessionId }: MockPlan): PlanSummary {
    const { plan, tasks } = work;
    const count = (type: string) => tasks.filter((t) => t.status.type === type).length;
    const attached = [...this.chats.values()].find((c) => c.planId === plan.planId);
    return {
      planId: plan.planId,
      projectRoot: plan.projectRoot,
      goal: plan.goal,
      status: plan.status,
      depth: plan.depth,
      revision: plan.revision,
      tasksTotal: tasks.length - count("proposed") - count("cancelled") - count("superseded"),
      tasksDone: count("done"),
      blocked: count("blocked"),
      needsYou: needsYouCount(needsYouItems(fromWorkSnapshot(work, null))),
      running: count("active"),
      lastSessionId: lastSessionId && this.chats.has(lastSessionId) ? lastSessionId : null,
      attachedTo: attached?.info.sessionId ?? null,
      createdAt: plan.createdAt,
      updatedAt: plan.updatedAt,
    };
  }

  /** `work_parallel`: ready tasks that may run beside the running ones. */
  parallel(sessionId: string): ParallelSet {
    const planId = this.chats.get(sessionId)?.planId;
    const work = planId ? this.plans.get(planId)?.work : undefined;
    if (!work) return { eligible: [], excluded: [], running: [], max: 3 };
    const done = new Set(work.tasks.filter((t) => t.status.type === "done").map((t) => t.id));
    const held = new Set(work.replanHeld);
    const ready = work.tasks.filter((t) => t.status.type === "pending" && !t.held && t.dependsOn.every((d) => done.has(d)));
    return {
      eligible: ready.filter((t) => !held.has(t.id)).map((t) => t.id),
      excluded: work.tasks.filter((t) => held.has(t.id)).map((t) => ({ taskId: t.id, block: { type: "replanning" } })),
      running: work.tasks.filter((t) => t.status.type === "active" && t.owner && t.owner.agentId !== "main").map((t) => t.id),
      max: 3,
    };
  }
}
