<script lang="ts">
  import type { ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo, ToolUseRef } from "$lib/domain/timeline/blocks";
  import { summarizeRun, workLine } from "$lib/domain/timeline/groups";
  import { resolveToolCall } from "$lib/domain/timeline/toolState";
  import Icon, { ChevronRight } from "$lib/ui/icons";

  /**
   * A finished turn's work in one line: how long it worked and what it did.
   * Clicking unfolds every step; failures already show below the line.
   */
  type Props = {
    uses: ToolUseRef[];
    results: Record<string, ToolResultInfo>;
    tools: Record<string, ToolCallView>;
    durationMs: number | null;
    /** Plan tasks that became done during the turn. */
    tasksDone?: number;
    open?: boolean;
  };
  let { uses, results, tools, durationMs, tasksDone = 0, open = $bindable(false) }: Props = $props();

  const run = $derived(summarizeRun(uses.map((use) => resolveToolCall(use, results[use.callId], tools[use.callId], false))));
  const line = $derived(workLine(run, durationMs, tasksDone));
</script>

<button
  type="button"
  class="work-summary"
  class:is-open={open}
  aria-expanded={open}
  title={open ? "Hide the steps" : "Show every step"}
  onclick={() => (open = !open)}
>
  <span class="work-summary-line">{line}</span>
  {#if run.failed}<span class="work-flag is-failed">{run.failed} failed</span>{/if}
  {#if run.denied}<span class="work-flag is-denied">{run.denied} denied</span>{/if}
  <span class="work-summary-chevron" aria-hidden="true"><Icon icon={ChevronRight} size={11} strokeWidth={2} /></span>
</button>
