<script lang="ts">
  import type { TaskMap } from "$lib/domain/work/status";
  import { whyLines } from "$lib/domain/work/why";
  import type { WhyStep } from "$lib/protocol/WhyStep";
  import { taskWhy } from "$lib/runtime";
  import { Disclosure } from "$lib/ui";
  import { relTime } from "$lib/util";

  /**
   * How the task came to be as it is: who created it and in which revision,
   * each change that touched it with its cause, reason and evidence, and the
   * discovery or experiment behind it. Loaded when opened, again after the
   * task changes.
   */
  type Props = { sessionId: string; taskId: string; tasks: TaskMap; revision: number };
  let { sessionId, taskId, tasks, revision }: Props = $props();

  type State = { kind: "loading" } | { kind: "ready"; steps: WhyStep[] } | { kind: "error"; error: string };
  let open = $state(false);
  let load = $state<State>({ kind: "loading" });
  const lines = $derived(load.kind === "ready" ? whyLines(load.steps, tasks) : []);

  $effect(() => {
    if (!open) return;
    void revision;
    let live = true;
    load = { kind: "loading" };
    void taskWhy(sessionId, taskId).then((r) => {
      if (live) load = r.ok ? { kind: "ready", steps: r.value } : { kind: "error", error: r.error };
    });
    return () => {
      live = false;
    };
  });
</script>

<Disclosure class="task-why" summaryClass="task-why-summary" bind:open>
  {#snippet summary()}
    <span class="plan-view-heading">Why?</span>
  {/snippet}
  {#if load.kind === "loading"}
    <p class="task-note">Loading the history…</p>
  {:else if load.kind === "error"}
    <p class="task-note">Not available · {load.error}</p>
  {:else if lines.length === 0}
    <p class="task-note">Nothing recorded yet.</p>
  {:else}
    <ol class="task-why-steps">
      {#each lines as line (line.key)}
        <li>
          <div class="task-why-head">
            <span class="task-why-who">{line.who}</span>
            <span>{line.what}</span>
            <span class="task-why-when">rev {line.revision} · {relTime(line.at)}</span>
          </div>
          {#if line.cause}<p class="task-why-detail">via {line.cause}</p>{/if}
          {#each line.because as text, i (i)}<p class="task-why-detail">{text}</p>{/each}
          {#if line.evidence.length}<p class="task-why-detail">Evidence: {line.evidence.join(" · ")}</p>{/if}
        </li>
      {/each}
    </ol>
  {/if}
</Disclosure>
