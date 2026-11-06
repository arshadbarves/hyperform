<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import type { PlanPanel } from "$lib/domain/work/panel";
  import { keepPlanning, type BuildTarget } from "$lib/runtime";
  import { startBuild } from "$lib/stores/app-actions";
  import { Button, Menu, buttonClass } from "$lib/ui";
  import Icon, { ChevronDown } from "$lib/ui/icons";
  import PlanFeedback from "./PlanFeedback.svelte";

  /**
   * The Draft's footer: Build (here, in a new chat that gets the plan
   * without the research, or here with helpers in parallel when tasks can
   * run together) or Keep planning with feedback. Replanning has the change
   * set instead; paused, done and abandoned plans their state banner.
   * Building has no footer; the header's menu pauses or stops it.
   */
  type Props = {
    work: WorkView;
    panel: PlanPanel;
    requestId: string | null;
    /** Tasks that could run together once approved; 0 hides the parallel build. */
    parallel: number;
  };
  let { work, panel, requestId, parallel }: Props = $props();

  let busy = $state(false);
  let revising = $state(false);
  const planId = $derived(work.plan.planId);

  async function run(job: () => Promise<boolean>) {
    if (busy) return;
    busy = true;
    try {
      if (await job()) revising = false;
    } finally {
      busy = false;
    }
  }

  const build = (target: BuildTarget, inParallel = false) =>
    run(() => startBuild({ planId, projectRoot: work.plan.projectRoot, target, requestId, parallel: inParallel }));
</script>

{#if panel.kind === "draft"}
  <footer class="plan-view-actions">
    {#if revising}
      <PlanFeedback {busy} onSend={(text) => void run(() => keepPlanning(requestId, text))} onCancel={() => (revising = false)} />
    {:else}
      <div class="plan-view-buttons">
        <div class="plan-build">
          <Button variant="accent" disabled={busy} title="Stop planning and build it in this chat" onclick={() => void build("here")}>
            Build
          </Button>
          <Menu.Root>
            <Menu.Trigger class={buttonClass("accent", { className: "plan-build-more" })} aria-label="More ways to build" disabled={busy}>
              <Icon icon={ChevronDown} size={12} strokeWidth={2} />
            </Menu.Trigger>
            <Menu.Portal>
              <Menu.Content class="menu" side="top" align="start" sideOffset={6}>
                <Menu.Item class="menu-item is-stacked" onSelect={() => void build("here")}>
                  <span class="menu-item-label">Build here</span>
                  <span class="menu-item-sub">This chat keeps its research and builds</span>
                </Menu.Item>
                <Menu.Item class="menu-item is-stacked" onSelect={() => void build("newChat")}>
                  <span class="menu-item-label">Build in a new chat</span>
                  <span class="menu-item-sub">A fresh chat gets the plan, not the research</span>
                </Menu.Item>
                {#if parallel >= 2}
                  <Menu.Item class="menu-item is-stacked" onSelect={() => void build("here", true)}>
                    <span class="menu-item-label">Build with helpers in parallel</span>
                    <span class="menu-item-sub">{parallel} tasks can run together</span>
                  </Menu.Item>
                {/if}
              </Menu.Content>
            </Menu.Portal>
          </Menu.Root>
        </div>
        <Button disabled={busy} onclick={() => (revising = true)}>Keep planning</Button>
      </div>
    {/if}
  </footer>
{/if}
