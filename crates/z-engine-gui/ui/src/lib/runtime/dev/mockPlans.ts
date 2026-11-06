import { agentInfo } from "../../domain/testFixtures";
import { criterion, entry, finding, header, health, task } from "../../domain/work/fixtures";
import type { AgentInfo } from "../../protocol/AgentInfo";
import type { Criterion } from "../../protocol/Criterion";
import type { PlanHeader } from "../../protocol/PlanHeader";
import type { PlanHealth } from "../../protocol/PlanHealth";
import type { Task } from "../../protocol/Task";
import type { WorkSnapshot } from "../../protocol/WorkSnapshot";

/**
 * The mock backend's plans, one per Plan tab state: Draft, Building with
 * helpers (one running, one ready to apply; a medium and a low finding that
 * stay out of Needs you), Replanning with held tasks and a high finding,
 * Paused (an interrupted task whose helper's work is kept, a held one and a
 * helper still stopping), Done, plus a plan detached from its chat and an
 * abandoned one with no chat, for the picker and the home Continue card.
 */

export const PROJECT = "/Users/you/notes-app";
export const SITE = "/Users/you/website";
const WT = `${PROJECT}/.z-engine/worktrees`;

export interface MockPlan {
  work: WorkSnapshot;
  lastSessionId: string | null;
}

export function healthOf(tasks: Task[]): PlanHealth {
  const count = (type: Task["status"]["type"]) => tasks.filter((t) => t.status.type === type).length;
  return health({
    tasks: tasks.length,
    tasksDone: count("done"),
    blocked: count("blocked"),
    failed: count("failed"),
    tasksWithoutCriteria: tasks.filter((t) => t.acceptance.length === 0).length,
  });
}

function snap(plan: PlanHeader, tasks: Task[], over: Partial<WorkSnapshot> = {}): WorkSnapshot {
  return { plan, tasks, entries: [], findings: [], audit: [], health: healthOf(tasks), replanHeld: [], ...over };
}

const met = (id: string, text: string, method: Criterion["method"], label = ""): Criterion =>
  criterion(id, {
    text,
    method,
    status: "met",
    evidence: label
      ? [{ type: "check", sessionId: "c-done", recordId: `r-${id}`, passed: true, fingerprint: "f", kind: "test", label, at: 0 }]
      : [],
  });
const open = (id: string, text: string): Criterion => criterion(id, { text });

function helper(agentId: string, description: string, worktree: string, over: Partial<AgentInfo> = {}): AgentInfo {
  return agentInfo({
    agentId,
    agentType: "general-purpose",
    description,
    isolation: "worktree",
    worktree: { path: worktree, branch: `z/${agentId}`, base: "main", state: "pending", filesChanged: 3, diffstat: "+84 −12" },
    toolCalls: 12,
    ...over,
  });
}

function draft(now: number): MockPlan {
  const at = { createdAt: now - 60 * 60_000, updatedAt: now - 50 * 60_000 };
  const plan = header({
    planId: "P-draft",
    projectRoot: PROJECT,
    goal: "Add offline sync to the notes app",
    document: "# Offline sync\n\nEdits made offline queue locally and replay in order once the app is back online.",
    ...at,
  });
  return {
    lastSessionId: "c-draft",
    work: snap(plan, [
      task("q1", { title: "Store edits in a local queue", acceptance: [open("q1a", "Edits survive a reload")], order: 0, ...at }),
      task("q2", { title: "Replay the queue when online", dependsOn: ["q1"], acceptance: [open("q2a", "Replays in order")], order: 1, ...at }),
      task("q3", { title: "Show the sync state in the toolbar", order: 2, ...at }),
    ]),
  };
}

function building(now: number): { plan: MockPlan; agents: AgentInfo[] } {
  const at = { createdAt: now - 3 * 3600_000, updatedAt: now - 2 * 60_000 };
  const plan = header({ planId: "P-build", projectRoot: PROJECT, goal: "Build a full-text search index", status: "active", parallel: "approved", revision: 5, ...at });
  const owner = (agentId: string) => ({ sessionId: "c-build", agentId });
  const searchBox = `${WT}/search-box`;
  const run = (agentId: string, path: string, integration: Task["run"]["integration"] = null) => ({
    dispatch: { sessionId: "c-build", agentId, worktree: path, attempt: 1, at: now - 6 * 60_000 },
    integration,
    report: null,
    forgivenAttempts: 0,
    serial: false,
  });
  const tasks = [
    task("s1", { title: "Pick the tokenizer", status: { type: "done" }, result: "Unicode word splitting with stemming.", order: 0, acceptance: [met("s1a", "Splits CJK text", { type: "inspection" })], ...at }),
    task("s2", { title: "Index notes on save", activeForm: "Indexing notes on save", status: { type: "active" }, owner: owner("agt-index"), run: run("agt-index", `${WT}/index-on-save`), order: 1, acceptance: [open("s2a", "Saving a note updates the index")], ...at }),
    task("s3", { title: "Search box with highlighting", activeForm: "Building the search box", status: { type: "active" }, owner: owner("agt-ui"), run: run("agt-ui", searchBox, { type: "pendingApply", worktree: searchBox }), order: 2, ...at }),
    task("s4", { title: "Rank results by recency", dependsOn: ["s2"], order: 3, ...at }),
    task("s5", { title: "Add a search keyboard shortcut", status: { type: "proposed" }, order: 4, ...at }),
  ];
  const audit = [
    finding("a2", { text: "“Rank results by recency” has no acceptance criteria", refs: [{ type: "task", taskId: "s4" }] }),
    finding("a3", { severity: "low", text: "“Search box with highlighting” has no acceptance criteria", refs: [{ type: "task", taskId: "s3" }] }),
  ];
  return {
    plan: { lastSessionId: "c-build", work: snap(plan, tasks, { audit }) },
    agents: [
      helper("agt-index", "Index notes on save", `${WT}/index-on-save`, { taskId: "s2", startedAt: now - 6 * 60_000 }),
      helper("agt-ui", "Search box with highlighting", searchBox, { taskId: "s3", status: "completed", startedAt: now - 5 * 60_000, finishedAt: now - 3 * 60_000 }),
    ],
  };
}

function replanning(now: number): MockPlan {
  const at = { createdAt: now - 5 * 3600_000, updatedAt: now - 15 * 60_000 };
  const plan = header({ planId: "P-replan", projectRoot: PROJECT, goal: "Move payments to the new billing API", status: "replanning", revision: 7, ...at });
  const since = { revision: 7, createdRevision: 2 };
  const tasks = [
    task("b1", { title: "Wrap the billing client", status: { type: "done" }, order: 0, ...at }),
    task("b2", { title: "Migrate subscriptions", status: { type: "failed", reason: "The webhook signature changed" }, order: 1, ...at, ...since }),
    task("b3", { title: "Migrate invoices", dependsOn: ["b2"], order: 2, ...at }),
    task("b4", { title: "Refund flow", dependsOn: ["b3"], order: 3, ...at }),
    task("b5", { title: "Update the billing docs", order: 4, ...at }),
    task("b6", { title: "Verify webhook signatures", order: 5, ...at, revision: 7, createdRevision: 7 }),
  ];
  const change = entry(
    "e-replan",
    {
      type: "planChange",
      cause: "replan",
      source: null,
      original: "Migrate subscriptions, then invoices",
      new: "Verify webhook signatures first",
      reason: "Subscriptions failed on the new webhook signature; invoices and refunds wait for the fix.",
      evidence: [],
      affected: ["b2", "b3", "b4", "b6"],
    },
    { createdAt: now - 15 * 60_000, createdRevision: 7, revision: 7 },
  );
  return {
    lastSessionId: "c-replan",
    work: snap(plan, tasks, {
      entries: [change],
      findings: [{ type: "dependencyFailed", taskId: "b3", dependsOn: "b2" }],
      audit: [finding("a1", { severity: "high", text: "“Refund flow” has no acceptance criteria", refs: [{ type: "task", taskId: "b4" }] })],
      replanHeld: ["b3", "b4"],
    }),
  };
}

function paused(now: number): { plan: MockPlan; agents: AgentInfo[] } {
  const at = { createdAt: now - 26 * 3600_000, updatedAt: now - 30 * 60_000 };
  const plan = header({ planId: "P-paused", projectRoot: PROJECT, goal: "Migrate settings to the new schema", status: "paused", parallel: "approved", revision: 9, ...at });
  const convert = `${WT}/convert-settings`;
  const tasks = [
    task("m1", { title: "Write the new schema", status: { type: "done" }, order: 0, ...at }),
    task("m2", {
      title: "Convert old settings files",
      order: 1,
      resume: { sessionId: "c-paused", agentId: "agt-convert", worktree: convert, note: "Converted 14 of 20 files" },
      ...at,
    }),
    task("m3", { title: "Drop the v1 reader", held: true, dependsOn: ["m2"], order: 2, ...at }),
    task("m4", { title: "Validate settings on load", activeForm: "Validating settings on load", status: { type: "active" }, owner: { sessionId: "c-paused", agentId: "agt-validate" }, order: 3, ...at }),
  ];
  const resumed = entry(
    "e-resume",
    {
      type: "planChange",
      cause: "resume",
      source: null,
      original: "",
      new: "",
      reason: "Resumed: 1 task was interrupted, 1 file changed outside the plan",
      evidence: [],
      affected: ["m2"],
    },
    { createdAt: now - 31 * 60_000, createdRevision: 9, revision: 9 },
  );
  return {
    plan: { lastSessionId: "c-paused", work: snap(plan, tasks, { entries: [resumed] }) },
    agents: [
      helper("agt-convert", "Convert old settings files", convert, { taskId: "m2", status: "cancelled", finishedAt: now - 40 * 60_000 }),
      helper("agt-validate", "Validate settings on load", `${WT}/validate`, { taskId: "m4", startedAt: now - 35 * 60_000 }),
    ],
  };
}

function done(now: number): MockPlan {
  const at = { createdAt: now - 4 * 86_400_000, updatedAt: now - 2 * 86_400_000 };
  const plan = header({ planId: "P-done", projectRoot: PROJECT, goal: "Dark mode for the editor", status: "done", revision: 6, ...at });
  return {
    lastSessionId: "c-done",
    work: snap(plan, [
      task("d1", { title: "Theme tokens for dark", status: { type: "done" }, acceptance: [met("d1a", "Tokens cover every surface", { type: "check", kind: "test" }, "npm test")], ...at }),
      task("d2", { title: "Follow the system setting", status: { type: "done" }, acceptance: [met("d2a", "Switches with the OS", { type: "inspection" })], ...at }),
      task("d3", { title: "Contrast pass", status: { type: "done" }, acceptance: [met("d3a", "Reads well at night", { type: "user" })], ...at }),
      task("d4", { title: "High-contrast variant", status: { type: "cancelled" }, ...at }),
    ]),
  };
}

/** Plans no chat has: one detached from the chat that had it last, one abandoned. */
function unattached(now: number): MockPlan[] {
  const docsAt = { createdAt: now - 8 * 3600_000, updatedAt: now - 3 * 3600_000 };
  const oldAt = { createdAt: now - 9 * 86_400_000, updatedAt: now - 5 * 86_400_000 };
  const docs = header({ planId: "P-docs", projectRoot: PROJECT, goal: "Refresh the user guide", status: "active", revision: 4, ...docsAt });
  const gql = header({ planId: "P-gql", projectRoot: PROJECT, goal: "Try a GraphQL gateway", status: "abandoned", revision: 3, ...oldAt });
  return [
    {
      lastSessionId: "c-guide",
      work: snap(docs, [
        task("g1", { title: "Outline the new chapters", status: { type: "done" }, ...docsAt }),
        task("g2", { title: "Rewrite getting started", order: 1, ...docsAt }),
        task("g3", { title: "Screenshots for every page", order: 2, ...docsAt }),
      ]),
    },
    { lastSessionId: null, work: snap(gql, [task("x1", { title: "Schema stitching spike", ...oldAt })]) },
  ];
}

/** Every plan, and the helpers each chat shows in its Agents tab. */
export function mockPlans(now: number): { plans: MockPlan[]; agents: Record<string, AgentInfo[]> } {
  const build = building(now);
  const pause = paused(now);
  return {
    plans: [draft(now), build.plan, replanning(now), pause.plan, done(now), ...unattached(now)],
    agents: { "c-build": build.agents, "c-paused": pause.agents },
  };
}
