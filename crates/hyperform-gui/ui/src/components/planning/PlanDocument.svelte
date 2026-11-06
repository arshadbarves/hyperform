<script lang="ts">
  import { untrack } from "svelte";
  import { panelPlan } from "$lib/domain/plans";
  import { MAIN_AGENT, type WorkView } from "$lib/domain/sessionView";
  import { modeMeta } from "$lib/domain/modes";
  import { todoProgress } from "$lib/domain/todos";
  import type { PlanDecision } from "$lib/protocol/PlanDecision";
  import { editWork, resolvePlan, sessions } from "$lib/runtime";
  import { Button, EmptyState, Pill } from "$lib/ui";
  import Icon, { Eye, ListChecks, Pencil } from "$lib/ui/icons";
  import Markdown from "../chat/Markdown.svelte";
  import TodoChecklist from "./TodoChecklist.svelte";

  /**
   * The plan document. Without a work plan (`work` unset) this is the whole
   * Plan tab: a plan waiting for review, with its checklist and Build / Keep
   * planning; with nothing pending, the chat's last plan. A pending plan can
   * be edited before it is approved. Build ends plan mode and keeps the
   * permission level shown in the composer. With a work plan it is the
   * Document view: the plan's markdown, editable while Draft, saved as a
   * revision; the footer below it builds.
   */
  type Props = { work?: WorkView | null; editable?: boolean };
  let { work = null, editable = false }: Props = $props();

  const view = $derived(sessions.active);
  const shown = $derived(panelPlan(view));
  const pending = $derived(shown?.kind === "pending" ? shown.pending : null);
  const text = $derived(shown ? (shown.kind === "pending" ? shown.pending.plan : shown.plan) : "");
  const document = $derived(work ? work.plan.document || text : text);
  const progress = $derived(todoProgress(shown?.todos));
  const agentLabel = $derived.by(() => {
    if (!pending || pending.agentId === MAIN_AGENT) return null;
    const info = view?.agents[pending.agentId];
    return info ? `${info.agentType} · ${info.description}` : pending.agentId;
  });

  let revising = $state(false);
  let feedback = $state("");
  let sending = $state(false);
  let editing = $state(false);
  let draft = $state("");
  const canEdit = $derived(work ? editable : pending !== null);
  const base = $derived(work ? document : (pending?.plan ?? ""));
  const edited = $derived(canEdit && draft.trim() !== base.trim());
  const requestId = $derived(pending?.requestId ?? null);
  const level = $derived(modeMeta(view?.mode ?? "default").label);

  // Each plan under review (or saved work plan document) starts unedited, without a reply half written.
  $effect(() => {
    void requestId;
    void work?.plan.document;
    untrack(() => {
      draft = base;
      revising = false;
      editing = false;
      feedback = "";
      sending = false;
    });
  });

  async function decide(decision: PlanDecision) {
    if (!pending || sending) return;
    sending = true;
    if (!(await resolvePlan(pending.requestId, decision))) sending = false;
  }

  function build() {
    void decide({ type: "approve", mode: null, editedPlan: edited ? draft : null, handoff: false });
  }

  function keepPlanning() {
    if (!revising) {
      revising = true;
      return;
    }
    void decide({ type: "revise", feedback: feedback.trim() });
  }

  async function save() {
    if (!work || !edited || sending) return;
    sending = true;
    if (await editWork(work.plan.planId, { type: "editDocument", document: draft })) editing = false;
    sending = false;
  }
</script>

<div class="plan-view" class:is-embedded={work !== null}>
  {#if !work && !shown}
    <EmptyState
      icon={ListChecks}
      title="No plan yet"
      description="In Plan mode the agent researches first and proposes a plan here. Nothing changes until you approve it."
    />
  {:else if work && !document && !editing}
    <EmptyState icon={ListChecks} title="No document yet" description="The plan's tasks are its structure; a document appears when the agent writes one.">
      {#if canEdit}<Button size="s" onclick={() => (editing = true)}>Write one</Button>{/if}
    </EmptyState>
  {:else}
    <header class="plan-view-head">
      {#if !work}
        <span class={`plan-view-kicker${pending ? " is-pending" : ""}`}>{pending ? "Ready for review" : "Last plan"}</span>
        {#if agentLabel}<Pill tone="info">{agentLabel}</Pill>{/if}
      {/if}
      {#if edited}<Pill>Edited</Pill>{/if}
      <span class="plan-view-space"></span>
      {#if !work && progress.total}<span class="plan-view-progress">{progress.done} of {progress.total} done</span>{/if}
      {#if work && edited}
        <Button size="s" variant="primary" disabled={sending} onclick={() => void save()}>Save</Button>
      {/if}
      {#if canEdit}
        <Button size="s" active={editing} aria-pressed={editing} disabled={sending} onclick={() => (editing = !editing)}>
          <Icon icon={editing ? Eye : Pencil} size={13} />
          {editing ? "Preview" : "Edit"}
        </Button>
      {/if}
    </header>

    <div class="plan-view-body">
      {#if canEdit && editing}
        <textarea
          class="text-field plan-view-editor"
          bind:value={draft}
          spellcheck={false}
          aria-label={work ? "Edit the plan document" : "Edit the plan before approving it"}
          onkeydown={(e) => {
            if (e.key === "s" && (e.metaKey || e.ctrlKey) && work) {
              e.preventDefault();
              void save();
            }
            if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              editing = false;
            }
          }}
        ></textarea>
      {:else}
        <div class="plan-view-text"><Markdown text={canEdit ? draft : document} /></div>
      {/if}
      {#if !work && shown?.todos.length}
        <section class="plan-view-todos" aria-label="Checklist">
          <h3 class="plan-view-heading">Checklist</h3>
          <TodoChecklist todos={shown.todos} />
        </section>
      {/if}
    </div>

    {#if !work && pending}
      <footer class="plan-view-actions">
        {#if revising}
          <!-- svelte-ignore a11y_autofocus -->
          <textarea
            class="plan-view-feedback"
            bind:value={feedback}
            rows={3}
            autofocus
            placeholder="What should change in the plan?"
            onkeydown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) keepPlanning();
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                revising = false;
              }
            }}
          ></textarea>
        {/if}
        <div class="plan-view-buttons">
          <Button variant="accent" disabled={sending} title={`Stop planning and implement it · ${level}`} onclick={build}>Build</Button>
          <Button disabled={sending} onclick={keepPlanning}>{revising ? "Send feedback" : "Keep planning"}</Button>
        </div>
      </footer>
    {/if}
  {/if}
</div>
