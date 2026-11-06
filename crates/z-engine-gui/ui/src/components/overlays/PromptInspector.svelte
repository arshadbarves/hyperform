<script lang="ts">
  import { inspectRequest } from "$lib/commands";
  import { outlineGroups, stepSelection } from "$lib/domain/inspectOutline";
  import { parseInspectRequest, type PromptInspect } from "$lib/domain/requestInspect";
  import { categorizeRow, inspectCopyText, inspectRows, type ContextCategory } from "$lib/promptInspectView";
  import { errorText, sessions } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button, EmptyState } from "$lib/ui";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Brain, Check, Copy } from "$lib/ui/icons";
  import InspectorDecisions from "./InspectorDecisions.svelte";
  import InspectorInsights from "./InspectorInsights.svelte";
  import InspectorMap from "./InspectorMap.svelte";
  import InspectorOutline from "./InspectorOutline.svelte";
  import InspectorPacket from "./InspectorPacket.svelte";
  import InspectorReader from "./InspectorReader.svelte";

  /**
   * The side panel's Context tab: the last request sent to the model, part
   * by part. A ring of what fills the context window and an outline to move
   * through; with the panel expanded, the selected part reads beside them,
   * and picking a part while docked expands the panel to read it.
   */
  type Props = { expanded: boolean };
  let { expanded }: Props = $props();

  let snap = $state<PromptInspect | null>(null);
  let err = $state<string | null>(null);
  let selected = $state(0);
  let query = $state("");
  let only = $state<ContextCategory | null>(null);
  const copied = copyFeedback();

  const rows = $derived(snap ? inspectRows(snap) : []);
  const groups = $derived(outlineGroups(rows, query, only));
  const visible = $derived(groups.flatMap((g) => g.items.map((i) => i.index)));
  const limit = $derived(sessions.active?.contextLimit || 200_000);
  const totals = $derived.by(() => {
    const t: Record<ContextCategory, number> = { instructions: 0, project: 0, conversation: 0, capabilities: 0 };
    for (const row of rows) t[categorizeRow(row)] += row.kind === "msg" ? row.part.tokens : row.tool.tokens;
    return t;
  });

  $effect(() => {
    const id = sessions.activeId;
    snap = null;
    err = null;
    if (!id) {
      err = "Open a chat and send a message; its request shows up here.";
      return;
    }
    inspectRequest(id)
      .then((raw) => {
        snap = parseInspectRequest(raw);
        err = snap ? null : "No request yet. Send a message first.";
        selected = 0;
      })
      .catch((e: unknown) => (err = errorText(e)));
  });

  // What is selected stays visible when the outline is filtered.
  $effect(() => {
    if (visible.length && !visible.includes(selected)) selected = visible[0] ?? 0;
  });

  function pick(index: number) {
    selected = index;
    if (!expanded) ui.setPanelExpanded(true);
  }

  function step(dir: -1 | 1) {
    const next = stepSelection(visible, selected, dir);
    if (next !== null) selected = next;
  }
</script>

<div class="inspector-panel" class:is-expanded={expanded}>
  <header class="inspector-bar">
    {#if snap}<span class="inspector-model" title={snap.model}>{snap.model}</span>{/if}
    <span class="inspector-bar-space"></span>
    <Button size="s" disabled={!snap} onclick={() => snap && void copied.copy(inspectCopyText(snap))}>
      <Icon icon={copied.copied ? Check : Copy} size={13} />
      {copied.copied ? "Copied" : "Copy all"}
    </Button>
  </header>

  {#if err}
    <div class="inspector-state"><EmptyState icon={Brain} title="Nothing to inspect yet" description={err} /></div>
  {:else if !snap}
    <p class="inspector-state inspector-loading">Reading the last request…</p>
  {:else}
    <div class="inspector-body">
      <aside class="inspector-side">
        <InspectorMap {totals} used={snap.totalTokens} {limit} active={only} onPick={(c) => (only = c)} />
        <InspectorOutline {groups} {selected} {query} total={rows.length} onQuery={(q) => (query = q)} onSelect={pick} onStep={step} />
        <InspectorInsights {snap} />
        <InspectorPacket />
        <InspectorDecisions />
      </aside>
      {#if expanded}
        <section class="inspector-main">
          <InspectorReader row={rows[selected]} totalTokens={snap.totalTokens} />
        </section>
      {/if}
    </div>
  {/if}
</div>
