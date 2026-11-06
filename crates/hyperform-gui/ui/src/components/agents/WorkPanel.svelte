<script lang="ts">
  import { agentActivity, agentSections, agentTree, agentUsageRows, workCounts } from "$lib/domain/agentTree";
  import { interruptedHelper } from "$lib/domain/work/interrupted";
  import { helperTask } from "$lib/domain/work/now";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { Task } from "$lib/protocol/Task";
  import { sessions } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { Badge, Button, Disclosure, EmptyState } from "$lib/ui";
  import Icon, { Bot, ChevronLeft, SquareTerminal } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import AgentRow from "./AgentRow.svelte";
  import AgentTranscript from "./AgentTranscript.svelte";
  import AgentUsage from "./AgentUsage.svelte";
  import ApplyCard from "./ApplyCard.svelte";
  import InterruptedCard from "./InterruptedCard.svelte";
  import JobRow from "./JobRow.svelte";

  /**
   * The side panel's Agents tab: work waiting for Apply first, then work
   * kept for an interrupted plan task (Discard only), then running helpers
   * with what each does now and the plan task it was sent for; finished
   * ones and usage fold away. Background jobs have their own view beside
   * the helpers.
   */
  const view = $derived(sessions.active);
  const agents = $derived(view?.agents ?? {});
  const jobs = $derived(view?.jobs ?? {});
  const work = $derived(view?.work ?? null);
  const tree = $derived(agentTree(agents));
  const sections = $derived(agentSections(tree, work));
  const jobList = $derived(Object.values(jobs).sort((a, b) => b.startedAt - a.startedAt));
  const counts = $derived(workCounts(agents, jobs, work));
  const usageRows = $derived(view ? agentUsageRows(agents, view.agentUsage, view.costUsd) : []);
  const focused = $derived(ui.agentTranscript ? (agents[ui.agentTranscript] ?? null) : null);
  const tab = $derived(ui.workTab);
  const clock = ticker(() => counts.runningAgents + counts.runningJobs > 0);
  let finishedOpen = $state(false);
  let usageOpen = $state(false);

  function openAgent(agentId: string) {
    ui.openPanel("agents", agentId);
  }

  const taskOf = (agent: AgentInfo) => helperTask(agent, work, view?.sessionId ?? null);

  function openTask(task: Task) {
    const planId = work?.plan.planId;
    if (planId) ui.openPlan(planId, { selected: task.id, view: "tasks", findings: false });
  }
</script>

<div class="work-panel">
  <header class="work-head">
    {#if focused}
      <Button variant="icon" aria-label="Back to agents" onclick={() => (ui.agentTranscript = null)}>
        <Icon icon={ChevronLeft} size={15} />
      </Button>
      <span class="work-title" title={focused.description}>{focused.agentType} · {focused.description}</span>
    {:else}
      <div class="work-tabs" role="tablist" aria-label="Background work">
        <button type="button" role="tab" class="work-tab" class:is-on={tab === "agents"} aria-selected={tab === "agents"} onclick={() => (ui.workTab = "agents")}>
          <Icon icon={Bot} size={13} />
          Agents
          {#if counts.runningAgents}<span class="work-live" aria-label="running"></span>{/if}
          <Badge count={tree.length} />
        </button>
        <button type="button" role="tab" class="work-tab" class:is-on={tab === "jobs"} aria-selected={tab === "jobs"} onclick={() => (ui.workTab = "jobs")}>
          <Icon icon={SquareTerminal} size={13} />
          Jobs
          {#if counts.runningJobs}<span class="work-live" aria-label="running"></span>{/if}
          <Badge count={jobList.length} />
        </button>
      </div>
    {/if}
  </header>

  <div class="work-body">
    {#if !view}
      <EmptyState icon={Bot} title="No chat open" description="Open a chat to see its helpers and background jobs." />
    {:else if focused}
      {#key focused.agentId}
        <AgentTranscript {view} agent={focused} onOpenAgent={openAgent} />
      {/key}
    {:else if tab === "agents"}
      {#if tree.length === 0}
        <EmptyState icon={Bot} title="No helpers yet" description="For broad work the agent sends helpers (subagents) off in parallel. They appear here." />
      {:else}
        {#if sections.ready.length}
          <section class="work-section" aria-label="Ready to apply">
            <h3 class="work-section-title">Ready to apply</h3>
            {#each sections.ready as node (node.info.agentId)}<ApplyCard agent={node.info} onOpen={openAgent} />{/each}
          </section>
        {/if}
        {#if sections.interrupted.length}
          <section class="work-section" aria-label="Interrupted">
            <h3 class="work-section-title">Interrupted</h3>
            {#each sections.interrupted as node (node.info.agentId)}
              {@const kept = interruptedHelper(node.info, work)}
              {#if kept}<InterruptedCard agent={node.info} {kept} onOpen={openAgent} onTask={openTask} />{/if}
            {/each}
          </section>
        {/if}
        {#if sections.working.length}
          <section class="work-section" aria-label="Working">
            <h3 class="work-section-title">Working</h3>
            {#each sections.working as node (node.info.agentId)}
              <AgentRow
                {node}
                now={clock.now}
                activity={agentActivity(view.tools, node.info.agentId)}
                task={taskOf(node.info)}
                onOpen={openAgent}
                onTask={openTask}
              />
            {/each}
          </section>
        {/if}
        {#if sections.finished.length}
          <Disclosure bind:open={finishedOpen} class="work-fold" summaryClass="work-fold-summary">
            {#snippet summary()}<span>Finished</span><Badge count={sections.finished.length} />{/snippet}
            <div class="work-section">
              {#each sections.finished as node (node.info.agentId)}
                <AgentRow {node} now={clock.now} activity={null} task={taskOf(node.info)} onOpen={openAgent} onTask={openTask} />
              {/each}
            </div>
          </Disclosure>
        {/if}
        {#if usageRows.length > 1}
          <Disclosure bind:open={usageOpen} class="work-fold" summaryClass="work-fold-summary">
            {#snippet summary()}<span>Usage by agent</span>{/snippet}
            <AgentUsage rows={usageRows} />
          </Disclosure>
        {/if}
      {/if}
    {:else if jobList.length === 0}
      <EmptyState icon={SquareTerminal} title="No background jobs" description="Commands the agent leaves running, and background helpers, appear here." />
    {:else}
      <div class="work-section">
        {#each jobList as job (job.jobId)}<JobRow {job} now={clock.now} onOpenAgent={openAgent} />{/each}
      </div>
    {/if}
  </div>
</div>
