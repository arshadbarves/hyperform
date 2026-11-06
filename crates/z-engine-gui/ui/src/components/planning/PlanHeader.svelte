<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import { coverageLine } from "$lib/domain/work/health";
  import { planActions, planCounts, statusWord, type PlanAction, type PlanPanel, type PlanViewMode } from "$lib/domain/work/panel";
  import { planProgress } from "$lib/domain/work/progress";
  import type { ParallelPolicy } from "$lib/protocol/ParallelPolicy";
  import { detachPlan, editWork } from "$lib/runtime";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { buttonClass, Menu, SegmentedChoice } from "$lib/ui";
  import Icon, { ChevronRight, MoreHorizontal } from "$lib/ui/icons";

  /**
   * The Plan tab's fixed header: the status word, the goal, progress (or,
   * under review, what the plan holds), the revision and the plan's menu
   * (Pause, Stop and Abandon as the state allows, switching to another of
   * the project's plans, detaching), the coverage line (which opens the
   * audit's findings), then the Tasks | Document switch.
   */
  type Props = {
    work: WorkView;
    panel: PlanPanel;
    /** The agent's plan waits for your review. */
    pending: boolean;
    /** The plan's parallel policy while `work_parallel` is effective, else null. */
    parallel: ParallelPolicy | null;
    /** A task is still bound to an agent, so a paused plan can still be stopped. */
    bound: boolean;
    /** The project has other plans; the menu then offers to switch. */
    canSwitch: boolean;
    mode: PlanViewMode;
    onMode: (mode: PlanViewMode) => void;
    /** The revision opens the notebook's Changes. */
    onRevision: () => void;
    onCoverage: () => void;
    onSwitch: () => void;
  };
  let { work, panel, pending, parallel, bound, canSwitch, mode, onMode, onRevision, onCoverage, onSwitch }: Props = $props();

  const MODES = [
    { value: "tasks", label: "Tasks", description: "The plan's tasks and their status" },
    { value: "document", label: "Document", description: "The plan as written" },
  ] as const;

  const plan = $derived(work.plan);
  const word = $derived(statusWord(panel.kind, pending || panel.kind === "replanning"));
  const progress = $derived(planProgress(work.tasks));
  const coverage = $derived(work.health ? coverageLine(work.health, work.audit) : null);
  const actions = $derived(planActions(panel.kind, { parallel, bound, canSwitch }));
  const items = $derived([
    ...plan.requirements.filter((r) => r.status !== "retired"),
    ...plan.invariants.filter((r) => r.status !== "retired"),
  ]);
  let goalOpen = $state(false);

  const CONFIRM = {
    stop: {
      title: "Stop work on this plan?",
      description:
        "Helpers stop now and the tasks they hold go back to waiting; their worktrees, the plan and its tasks are kept. Resume picks up from there.",
      confirmLabel: "Stop",
    },
    abandon: {
      title: "Abandon this plan?",
      description: "Nothing more runs on it. It stays in the project's plans, read-only; you can reopen it later.",
      confirmLabel: "Abandon",
    },
  } as const;

  async function run(action: PlanAction) {
    if (action === "detach") {
      void detachPlan();
      return;
    }
    if (action === "switchPlan") {
      onSwitch();
      return;
    }
    if (action === "runOneAtATime") {
      void editWork(plan.planId, { type: "setParallel", parallel: "off" });
      return;
    }
    if (action === "abandon" || action === "stop") {
      if (!(await confirmStore.ask({ ...CONFIRM[action], tone: "danger" }))) return;
    }
    void editWork(plan.planId, { type: action });
  }
</script>

<header class="plan-head">
  <div class="plan-head-top">
    <span class={`plan-head-status tone-${word.tone}`}>{word.label}</span>
    <button type="button" class="plan-head-rev" title="What changed, in the notebook" onclick={onRevision}>rev {plan.revision}</button>
    <span class="plan-view-space"></span>
    {#if actions.length}
      <Menu.Root>
        <Menu.Trigger class={buttonClass("icon")} aria-label="Plan actions">
          <Icon icon={MoreHorizontal} size={15} />
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Content class="menu" side="bottom" align="end" sideOffset={6}>
            {#each actions as item (item.action)}
              <Menu.Item class="menu-item" onSelect={() => void run(item.action)}>
                <span class="menu-item-label">{item.label}</span>
              </Menu.Item>
            {/each}
          </Menu.Content>
        </Menu.Portal>
      </Menu.Root>
    {/if}
  </div>

  <button
    type="button"
    class="plan-head-goal"
    class:is-open={goalOpen}
    aria-expanded={items.length ? goalOpen : undefined}
    title={goalOpen ? undefined : plan.goal}
    onclick={() => (goalOpen = !goalOpen)}
  >
    {plan.goal}
  </button>
  {#if goalOpen && items.length}
    <ul class="plan-head-items" aria-label="Requirements and invariants">
      {#each items as item (item.id)}
        <li>
          <span class="plan-head-item-kind">{item.kind === "invariant" ? "Invariant" : "Requirement"}</span>
          <span>{item.text}</span>
          {#if item.status === "proposed"}<span class="plan-head-item-proposed">proposed</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if panel.reviewing || progress.total === 0}
    <p class="plan-head-meta">{planCounts(plan, progress.total)}</p>
  {:else}
    <div class="plan-head-progress">
      <span class="plan-head-bar" aria-hidden="true"><span style:scale={`${progress.done / progress.total} 1`}></span></span>
      <span class="plan-head-meta">{progress.done} of {progress.total} tasks</span>
    </div>
  {/if}

  {#if coverage}
    <button type="button" class={`plan-coverage tone-${coverage.tone}`} title="The audit's findings" onclick={onCoverage}>
      <span>{coverage.text}</span>
      <Icon icon={ChevronRight} size={11} strokeWidth={2} />
    </button>
  {/if}

  <SegmentedChoice compact label="Plan view" options={MODES} value={mode} onSelect={onMode} />
</header>
