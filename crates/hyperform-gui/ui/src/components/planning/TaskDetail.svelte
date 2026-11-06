<script lang="ts">
  import { MAIN_AGENT, type WorkView } from "$lib/domain/sessionView";
  import { holdEdit } from "$lib/domain/work/held";
  import { linkedEntries } from "$lib/domain/work/notebook";
  import { WEB_HINT, fromWeb, kindLabel, taskActions, taskLook } from "$lib/domain/work/status";
  import { ancestors } from "$lib/domain/work/tree";
  import type { Task } from "$lib/protocol/Task";
  import type { WorkEdit } from "$lib/protocol/WorkEdit";
  import { editWork, sessions } from "$lib/runtime";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button, Disclosure, Menu, Pill, buttonClass } from "$lib/ui";
  import Icon, { ChevronLeft, MoreHorizontal } from "$lib/ui/icons";
  import Markdown from "../chat/Markdown.svelte";
  import NotebookEntry from "./NotebookEntry.svelte";
  import TaskCriteria from "./TaskCriteria.svelte";
  import TaskNotices from "./TaskNotices.svelte";
  import TaskResult from "./TaskResult.svelte";
  import TaskWhy from "./TaskWhy.svelte";
  import TodoChecklist from "./TodoChecklist.svelte";
  import { PILL_TONE } from "./taskGlyph";

  /**
   * One task: where it sits, its status and what it says (TaskNotices:
   * why, proposals, rechecks, holds, the plan change, an interrupted run),
   * what it waits on, its result, its criteria with evidence, its steps,
   * scope, notes and linked notebook entries, how it came to be (Why?),
   * and what the agent was given for it. Hold and Release are in its menu.
   */
  type Props = {
    work: WorkView;
    task: Task;
    readOnly: boolean;
    /** The detail replaced the tree (narrow) or the findings (wide), so it offers the way back. */
    showBack: boolean;
    backLabel: string;
    onBack: () => void;
    onSelect: (taskId: string) => void;
  };
  let { work, task, readOnly, showBack, backLabel, onBack, onSelect }: Props = $props();

  const view = $derived(sessions.active);
  const planId = $derived(work.plan.planId);
  const look = $derived(taskLook(task, work.tasks, work.plan.status));
  const can = $derived(taskActions(task, look.state, readOnly));
  const crumbs = $derived(ancestors(task.id, work.tasks));
  const deps = $derived(task.dependsOn.flatMap((id) => (work.tasks[id] ? [work.tasks[id]] : [])));
  const requirements = $derived(work.plan.requirements.filter((r) => task.requirements.includes(r.id)));
  const linked = $derived(linkedEntries(work.entries, task.id));
  const notes = $derived([
    ...task.notes.assumptions.map((text) => ({ label: "Assumes", text })),
    ...task.notes.constraints.map((text) => ({ label: "Constraint", text })),
    ...task.notes.risks.map((text) => ({ label: "Risk", text })),
    ...(task.notes.rollback ? [{ label: "Rollback", text: task.notes.rollback }] : []),
  ]);
  const owner = $derived.by(() => {
    const o = task.owner;
    if (!o) return null;
    const here = o.sessionId === view?.sessionId;
    if (o.agentId === MAIN_AGENT) return { label: here ? "Main agent" : "Another chat", agentId: null };
    const agent = here ? view?.agents[o.agentId] : undefined;
    return { label: agent ? `${agent.agentType} · ${agent.description}` : "A helper", agentId: here ? o.agentId : null };
  });

  let renaming = $state(false);
  let title = $state("");

  function edit(change: WorkEdit) {
    void editWork(planId, change);
  }

  function rename() {
    const next = title.trim();
    renaming = false;
    if (next && next !== task.title) edit({ type: "renameTask", taskId: task.id, title: next });
  }

  async function cancel() {
    const ok = await confirmStore.ask({
      title: "Mark this task not needed?",
      description: `“${task.title}” is cancelled; tasks that wait on it can go ahead.`,
      confirmLabel: "Not needed",
      tone: "danger",
    });
    if (ok) edit({ type: "cancelTask", taskId: task.id });
  }

  const isGlob = (path: string) => /[*?[{]/.test(path);
</script>

<article class="task-detail" aria-label={task.title}>
  <header class="task-detail-head">
    {#if showBack}
      <Button variant="icon" aria-label={backLabel} title="{backLabel} · Esc" onclick={onBack}>
        <Icon icon={ChevronLeft} size={15} />
      </Button>
    {/if}
    <nav class="task-crumbs" aria-label="Parent tasks">
      {#each crumbs as parent (parent.id)}
        <button type="button" class="task-link" onclick={() => onSelect(parent.id)}>{parent.title}</button>
        <span aria-hidden="true">/</span>
      {/each}
    </nav>
    <span class="plan-view-space"></span>
    {#if can.rename || can.hold || can.release || can.retry || can.cancel}
      <Menu.Root>
        <Menu.Trigger class={buttonClass("icon")} aria-label="Task actions">
          <Icon icon={MoreHorizontal} size={15} />
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Content class="menu" side="bottom" align="end" sideOffset={6}>
            {#if can.rename}
              <Menu.Item class="menu-item" onSelect={() => ((title = task.title), (renaming = true))}>
                <span class="menu-item-label">Rename</span>
              </Menu.Item>
            {/if}
            {#if can.hold}
              <Menu.Item class="menu-item" onSelect={() => edit(holdEdit(task.id, true))}>
                <span class="menu-item-label">Hold</span>
              </Menu.Item>
            {/if}
            {#if can.release}
              <Menu.Item class="menu-item" onSelect={() => edit(holdEdit(task.id, false))}>
                <span class="menu-item-label">Release</span>
              </Menu.Item>
            {/if}
            {#if can.retry}
              <Menu.Item class="menu-item" onSelect={() => edit({ type: "retryTask", taskId: task.id })}>
                <span class="menu-item-label">Retry</span>
              </Menu.Item>
            {/if}
            {#if can.runAlone}
              <Menu.Item
                class="menu-item"
                onSelect={() => edit({ type: "setRunAlone", taskId: task.id, runAlone: !task.runAlone })}
              >
                <span class="menu-item-label">{task.runAlone ? "May run beside others" : "Run alone"}</span>
              </Menu.Item>
            {/if}
            {#if can.cancel}
              <Menu.Item class="menu-item" onSelect={() => void cancel()}>
                <span class="menu-item-label">Mark not needed</span>
              </Menu.Item>
            {/if}
          </Menu.Content>
        </Menu.Portal>
      </Menu.Root>
    {/if}
  </header>

  {#if renaming}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="text-field task-detail-rename"
      bind:value={title}
      autofocus
      aria-label="Task title"
      onblur={rename}
      onkeydown={(e) => {
        if (e.key === "Enter") rename();
        if (e.key === "Escape") {
          e.stopPropagation();
          renaming = false;
        }
      }}
    />
  {:else}
    <h2 class="task-detail-title">{task.title}</h2>
  {/if}

  <div class="task-detail-meta">
    <Pill dot tone={PILL_TONE[look.tone]} live={look.state === "active"}>{look.label}</Pill>
    <span>{kindLabel(task.kind)}</span>
    {#if fromWeb(task)}<span title={WEB_HINT}>from web</span>{/if}
    {#if task.runAlone}<span>runs alone</span>{/if}
    {#if owner}
      {#if owner.agentId}
        <button type="button" class="task-link" onclick={() => ui.openPanel("agents", owner.agentId)}>{owner.label}</button>
      {:else}
        <span>{owner.label}</span>
      {/if}
    {/if}
  </div>
  <TaskNotices {work} {task} {look} {can} {readOnly} onEdit={edit} />
  {#if deps.length}
    <p class="task-detail-deps">
      After
      {#each deps as dep, i (dep.id)}
        <button type="button" class="task-link" onclick={() => onSelect(dep.id)}>{dep.title}</button>{i < deps.length - 1 ? "," : ""}
      {/each}
    </p>
  {/if}
  {#if task.description}<div class="task-detail-text"><Markdown text={task.description} /></div>{/if}
  <TaskResult {planId} {task} {readOnly} />

  {#if requirements.length}
    <section class="task-section" aria-label="Requirements">
      <h3 class="plan-view-heading">Serves</h3>
      <ul class="task-plain-list">{#each requirements as r (r.id)}<li>{r.text}</li>{/each}</ul>
    </section>
  {/if}

  <TaskCriteria {planId} {task} findings={work.findings} {readOnly} />

  {#if task.steps.length}
    <section class="task-section" aria-label="Steps">
      <h3 class="plan-view-heading">Steps</h3>
      <TodoChecklist todos={task.steps} />
    </section>
  {/if}

  {#if task.scope.files.length || task.scope.symbols.length}
    <section class="task-section" aria-label="Scope">
      <h3 class="plan-view-heading">Scope</h3>
      <div class="task-chips">
        {#each task.scope.files as file (file)}
          {#if isGlob(file)}
            <span class="task-chip">{file}</span>
          {:else}
            <button type="button" class="task-chip" title="Open in Changes" onclick={() => ui.openPanel("changes", file, "session")}>{file}</button>
          {/if}
        {/each}
        {#each task.scope.symbols as symbol (symbol)}<span class="task-chip is-symbol">{symbol}</span>{/each}
      </div>
    </section>
  {/if}

  {#if notes.length}
    <Disclosure class="task-notes" summaryClass="task-notes-summary">
      {#snippet summary()}<span class="plan-view-heading">Notes · {notes.length}</span>{/snippet}
      <dl class="notebook-entry-facts">
        {#each notes as note, i (i)}<dt>{note.label}</dt><dd>{note.text}</dd>{/each}
      </dl>
    </Disclosure>
  {/if}

  {#if linked.length}
    <section class="task-section" aria-label="Linked notebook entries">
      <h3 class="plan-view-heading">Linked</h3>
      {#each linked as entry (entry.id)}
        <NotebookEntry {planId} {entry} tasks={work.tasks} {readOnly} />
      {/each}
    </section>
  {/if}

  {#if view}<TaskWhy sessionId={view.sessionId} taskId={task.id} tasks={work.tasks} revision={task.revision} />{/if}

  <button type="button" class="task-link task-detail-packet" onclick={() => ui.openPacket(task.id)}>
    What the agent was given for this task
  </button>
</article>
