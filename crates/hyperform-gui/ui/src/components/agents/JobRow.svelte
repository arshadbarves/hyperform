<script lang="ts">
  import { elapsed } from "$lib/domain/format";
  import type { JobInfo } from "$lib/protocol/JobInfo";
  import { killJob } from "$lib/runtime";
  import { Button, Disclosure } from "$lib/ui";
  import Icon, { Bot, SquareTerminal } from "$lib/ui/icons";
  import StatusChip from "./StatusChip.svelte";

  /** A background job: what runs, for how long, the last lines while it runs, all of it on request. */
  type Props = { job: JobInfo; now: number; onOpenAgent: (agentId: string) => void };
  let { job, now, onOpenAgent }: Props = $props();

  const TAIL = 6;
  let killing = $state(false);
  let open = $state(false);
  const running = $derived(job.status === "running");
  const tail = $derived((job.outputTail ?? "").replace(/\n+$/, "").split("\n").slice(-TAIL).join("\n"));

  async function kill() {
    killing = true;
    if (!(await killJob(job.jobId))) killing = false;
  }
</script>

<div class={`job-row status-${job.status}`}>
  <div class="job-row-head">
    <Icon icon={job.kind === "agent" ? Bot : SquareTerminal} size={13} class="job-kind-icon" />
    <span class="job-label" title={job.label}>{job.label}</span>
    <StatusChip status={job.status} />
  </div>
  <div class="job-row-meta">
    <span>{job.kind === "agent" ? "background agent" : "shell"}</span>
    <span>{elapsed(job.startedAt, job.finishedAt, now)}</span>
    {#if job.exitCode !== null}<span class={job.exitCode === 0 ? "" : "job-exit-bad"}>exit {job.exitCode}</span>{/if}
    <span class="job-row-spacer"></span>
    {#if job.agentId}
      <button type="button" class="job-link" onclick={() => job.agentId && onOpenAgent(job.agentId)}>Transcript</button>
    {/if}
    {#if running}
      <Button variant="danger" size="s" disabled={killing} onclick={() => void kill()}>
        {killing ? "Stopping…" : "Stop"}
      </Button>
    {/if}
  </div>
  {#if job.outputTail}
    {#if running}
      <pre class="job-tail">{tail}</pre>
    {:else}
      <Disclosure bind:open summaryClass="job-output-summary">
        {#snippet summary()}<span>Output</span>{/snippet}
        <pre class="job-output">{job.outputTail}</pre>
      </Disclosure>
    {/if}
  {/if}
</div>
