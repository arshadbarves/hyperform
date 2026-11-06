<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import type { PanelKind } from "$lib/domain/work/panel";
  import { stateBanner } from "$lib/domain/work/planState";
  import type { WorkEdit } from "$lib/protocol/WorkEdit";
  import { editWork } from "$lib/runtime";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { Button } from "$lib/ui";
  import Icon, { CheckCircle2, MinusCircle, Pause } from "$lib/ui/icons";

  /**
   * The calm banner over a paused, done or abandoned plan. Paused: nothing
   * new starts, helpers wind down (the Now lanes show them stopping);
   * Resume, and Stop while helpers still run. Done: what was finished and
   * how it was verified, with Reopen. Abandoned: read-only, with Reopen.
   */
  type Props = { work: WorkView; kind: PanelKind };
  let { work, kind }: Props = $props();

  const banner = $derived(stateBanner(kind, work));
  const ICON = { paused: Pause, done: CheckCircle2, abandoned: MinusCircle } as const;
  let busy = $state(false);

  async function edit(change: WorkEdit) {
    if (busy) return;
    busy = true;
    try {
      await editWork(work.plan.planId, change);
    } finally {
      busy = false;
    }
  }

  async function stop() {
    const ok = await confirmStore.ask({
      title: "Stop the helpers now?",
      description: "They stop without finishing their step and the tasks they hold go back to waiting; their worktrees are kept.",
      confirmLabel: "Stop",
      tone: "danger",
    });
    if (ok) await edit({ type: "stop" });
  }
</script>

{#if banner}
  <section class={`plan-state is-${banner.kind}`} aria-label={banner.title}>
    <span class="plan-state-icon" aria-hidden="true"><Icon icon={ICON[banner.kind]} size={15} /></span>
    <div class="plan-state-body">
      <p class="plan-state-title">{banner.title}</p>
      <p class="plan-state-text">{banner.text}</p>
      {#if banner.lines.length}
        <ul class="plan-state-lines">
          {#each banner.lines as line (line)}<li>{line}</li>{/each}
        </ul>
      {/if}
    </div>
    <div class="plan-state-actions">
      {#if banner.resume}
        <Button variant="primary" disabled={busy} onclick={() => void edit({ type: "resume" })}>Resume</Button>
      {/if}
      {#if banner.stop}<Button disabled={busy} onclick={() => void stop()}>Stop</Button>{/if}
      {#if banner.reopen}<Button disabled={busy} onclick={() => void edit({ type: "reopen" })}>Reopen</Button>{/if}
    </div>
  </section>
{/if}
