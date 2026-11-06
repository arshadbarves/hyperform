<script lang="ts">
  import { panelPlan } from "$lib/domain/plans";
  import { holdBadges } from "$lib/domain/work/held";
  import {
    backLabel,
    emptyPlanView,
    hasBound,
    planPanel,
    scrollPlace,
    showsLanes,
    wideLayout,
    workPlansOn,
    type PlanViewState,
  } from "$lib/domain/work/panel";
  import { parallelBadges, parallelCount, workParallelOn } from "$lib/domain/work/parallel";
  import { canPickPlan, planPicker } from "$lib/domain/work/picker";
  import { visibleRows } from "$lib/domain/work/tree";
  import type { ParallelSet } from "$lib/protocol/ParallelSet";
  import { editWork, parallelSet, projectPlans, sessionList, sessions } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import PlanChangeSet from "./PlanChangeSet.svelte";
  import PlanCoverage from "./PlanCoverage.svelte";
  import PlanDocument from "./PlanDocument.svelte";
  import PlanEmpty from "./PlanEmpty.svelte";
  import PlanFooter from "./PlanFooter.svelte";
  import PlanHeader from "./PlanHeader.svelte";
  import PlanNeedsYou from "./PlanNeedsYou.svelte";
  import PlanNotebook from "./PlanNotebook.svelte";
  import PlanNow from "./PlanNow.svelte";
  import PlanPicker from "./PlanPicker.svelte";
  import PlanStateBanner from "./PlanStateBanner.svelte";
  import TaskDetail from "./TaskDetail.svelte";
  import TaskTree from "./TaskTree.svelte";

  /**
   * The side panel's Plan tab. Without work plans (the feature off) it is the
   * plan document as it always was. With them: Empty (start planning, or
   * continue one of the project's plans) until a plan is attached, then the
   * plan's header, a banner for its state (paused, done, abandoned), what
   * needs you, what runs now, its tasks (or its document) and a footer:
   * Build while Draft, the change set while Replanning. The header's menu
   * can list the project's other plans in place of this one. A wide panel
   * shows a task's detail (or the findings) beside the tree; a narrow one
   * drills in (Esc steps back). With `work_parallel` effective, rows say
   * whether they can run beside others.
   */
  const view = $derived(sessions.active);
  const work = $derived(view?.work ?? null);
  const experimental = $derived(settingsStore.settings?.experimental);
  const panel = $derived(planPanel({ work, enabled: workPlansOn(experimental) }));
  const parallelOn = $derived(panel.kind !== "off" && workParallelOn(experimental));
  const shown = $derived(panelPlan(view));
  const requestId = $derived(shown?.kind === "pending" ? shown.pending.requestId : null);
  const planId = $derived(work?.plan.planId ?? null);
  const place = $derived(planId ? ui.plan.of(planId) : emptyPlanView());

  let width = $state(0);
  const wide = $derived(wideLayout(width, ui.panel.expanded));
  const tree = $derived(
    work
      ? visibleRows({
          tasks: work.tasks,
          plan: work.plan.status,
          expanded: place.expanded,
          showReplaced: place.showReplaced,
          review: panel.reviewing,
        })
      : null,
  );
  const selected = $derived(work && place.selected ? (work.tasks[place.selected] ?? null) : null);
  const holds = $derived(work ? holdBadges(work) : {});

  // The project's plans, to continue one here: in the Empty state, or in
  // place of the attached plan once its menu asks to switch.
  const sessionId = $derived(view?.sessionId ?? null);
  const root = $derived(view?.info?.projectRoot ?? null);
  const plans = $derived(panel.kind === "off" ? [] : projectPlans.of(root));
  const picking = $derived(work !== null && sessionId !== null && ui.plan.pickingFor === sessionId);
  const picker = $derived(planPicker(plans, { currentPlanId: planId, sessionId, chats: sessionList.items }));
  $effect(() => {
    if (root && (panel.kind === "empty" || picking)) void projectPlans.refresh(root);
  });

  // What could run in parallel helpers: a preview while Draft, the live set
  // while work runs. Keyed on values, not the view, which changes per event.
  const planning = $derived(view?.planning ?? false);
  const kind = $derived(panel.kind);
  let parallel = $state<ParallelSet | null>(null);
  $effect(() => {
    const wanted = parallelOn && work !== null && (kind === "draft" || kind === "building" || kind === "replanning");
    void planning;
    if (!sessionId || !wanted) {
      parallel = null;
      return;
    }
    let live = true;
    void parallelSet(sessionId).then((r) => {
      if (live) parallel = r.ok ? r.value : null;
    });
    return () => {
      live = false;
    };
  });
  const badges = $derived(work && parallelOn ? parallelBadges(parallel, work.tasks) : {});

  function patch(change: Partial<PlanViewState>) {
    if (planId) ui.plan.set(planId, change);
  }

  function select(taskId: string) {
    if (planId) ui.plan.select(planId, taskId);
  }

  const openFindings = () => patch({ view: "tasks", selected: null, findings: true });
  const back = () => planId && ui.plan.back(planId);
  const backTo = $derived(backLabel(place));

  // Each plan, and each place in it, keeps where you were scrolled while
  // you look at another; scrolling records it, arriving restores it.
  let body: HTMLDivElement | undefined = $state();
  const scrollAt = $derived(planId ? scrollPlace(place, wide) : null);
  $effect(() => {
    const el = body;
    if (el && planId && scrollAt) el.scrollTop = ui.plan.scrollOf(planId, scrollAt);
  });
  function onScroll() {
    if (body && planId && scrollAt) ui.plan.keepScroll(planId, scrollAt, body.scrollTop);
  }
</script>

{#if panel.kind === "off"}
  <PlanDocument />
{:else if !work || !tree || !view}
  <PlanEmpty planning={view?.planning ?? false} {picker} />
{:else if picking}
  <div class="plan-view plan-empty">
    <PlanPicker {picker} onBack={() => ui.plan.pick(null)} />
  </div>
{:else}
  <div class="plan-work" class:is-wide={wide} bind:clientWidth={width}>
    <PlanHeader
      {work}
      {panel}
      pending={requestId !== null}
      parallel={parallelOn ? work.plan.parallel : null}
      bound={hasBound(work)}
      canSwitch={canPickPlan(plans, planId)}
      mode={place.view}
      onMode={(mode) => patch({ view: mode })}
      onRevision={() => patch({ view: "tasks", selected: null, notebookOpen: true, notebookTab: "changes" })}
      onCoverage={openFindings}
      onSwitch={() => sessionId && ui.plan.pick(sessionId)}
    />
    <PlanStateBanner {work} kind={panel.kind} />
    {#if !panel.readOnly}<PlanNeedsYou {work} onSelect={select} onFindings={openFindings} />{/if}
    <div class="plan-work-body" bind:this={body} onscroll={onScroll}>
      {#if place.view === "document"}
        <PlanDocument {work} editable={panel.kind === "draft"} />
      {:else if selected && !wide}
        {#key selected.id}
          <TaskDetail {work} task={selected} readOnly={panel.readOnly} showBack backLabel={backTo} onBack={back} onSelect={select} />
        {/key}
      {:else if place.findings && !wide}
        <PlanCoverage {work} readOnly={panel.readOnly} onBack={back} onSelect={select} />
      {:else}
        <div class="plan-work-main">
          <div class="plan-work-tree">
            {#if showsLanes(panel.kind)}<PlanNow {view} stopping={panel.kind === "paused"} onSelect={select} />{/if}
            <TaskTree
              rows={tree.rows}
              replaced={tree.replaced}
              showReplaced={place.showReplaced}
              selected={place.selected}
              onSelect={select}
              onToggle={(taskId, open) => planId && ui.plan.setExpanded(planId, taskId, open)}
              onShowReplaced={(show) => patch({ showReplaced: show })}
              onAccept={panel.readOnly || !planId ? undefined : (taskId) => void editWork(planId, { type: "acceptTask", taskId })}
              {badges}
              {holds}
            />
            <PlanNotebook {work} {place} readOnly={panel.readOnly} onState={patch} />
          </div>
          {#if wide}
            <div class="plan-work-detail">
              {#if selected}
                {#key selected.id}
                  <TaskDetail {work} task={selected} readOnly={panel.readOnly} showBack={place.findings} backLabel={backTo} onBack={back} onSelect={select} />
                {/key}
              {:else if place.findings}
                <PlanCoverage {work} readOnly={panel.readOnly} onBack={back} onSelect={select} />
              {:else}
                <p class="task-note">Pick a task to see its criteria, steps and history.</p>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    </div>
    {#if panel.kind === "replanning"}
      <PlanChangeSet {work} {requestId} onSelect={select} />
    {:else if panel.kind === "draft"}
      <PlanFooter {work} {panel} {requestId} parallel={parallelOn ? parallelCount(parallel) : 0} />
    {/if}
  </div>
{/if}
