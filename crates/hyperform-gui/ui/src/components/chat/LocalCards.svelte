<script lang="ts">
  import type { TimelineItem } from "$lib/domain/timeline/turns";
  import { visibleText } from "$lib/domain/timeline/blocks";
  import { Disclosure } from "$lib/ui";
  import Icon, { ChevronRight, CornerDownLeft, Terminal } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import Markdown from "./Markdown.svelte";
  import PlanTraceRow from "./PlanTraceRow.svelte";
  import RouteChip from "./RouteChip.svelte";
  import TaskViewDivider from "./TaskViewDivider.svelte";

  /** Transcript cards that are not model output: steering, command output, errors, routes, compaction, task views, plan traces. */
  type Props = {
    item: Extract<TimelineItem, { kind: "steer" | "output" | "error" | "route" | "compaction" | "taskView" | "trace" }>;
  };
  let { item }: Props = $props();
</script>

{#if item.kind === "steer"}
  <div class="steer-note">
    <Icon icon={CornerDownLeft} size={11} class="steer-icon" />
    <span class="steer-label">Steered</span>
    <span class="steer-text">{visibleText(item.message)}</span>
  </div>
{:else if item.kind === "output"}
  <div class="command-output-card">
    <div class="command-output-head">
      <Icon icon={Terminal} size={11} />
      <span>/{item.output.name}</span>
    </div>
    <div class="command-output-body"><Markdown text={item.output.markdown} /></div>
  </div>
{:else if item.kind === "error"}
  <div class="msg error" role="alert">{item.error.message}</div>
{:else if item.kind === "route"}
  <RouteChip route={item.route} />
{:else if item.kind === "taskView"}
  <TaskViewDivider view={item.view} latest={item.latest} />
{:else if item.kind === "trace"}
  <PlanTraceRow trace={item.trace} />
{:else}
  <Disclosure class="compaction-divider" summaryClass="compaction-head" chevron={false}>
    {#snippet summary()}
      <span class="compaction-rule" aria-hidden="true"></span>
      <span class="compaction-label">
        Context compacted · {fmtTokens(item.marker.tokensBefore)} → {fmtTokens(item.marker.tokensAfter)}{item.restored
          ? " · plan restored"
          : ""}
      </span>
      <span aria-hidden="true">·</span>
      <span class="compaction-toggle">
        Summary
        <span class="disclosure-chevron" aria-hidden="true"><Icon icon={ChevronRight} size={10} strokeWidth={2} /></span>
      </span>
      <span class="compaction-rule" aria-hidden="true"></span>
    {/snippet}
    <div class="compaction-summary"><Markdown text={item.marker.summary} /></div>
  </Disclosure>
{/if}
