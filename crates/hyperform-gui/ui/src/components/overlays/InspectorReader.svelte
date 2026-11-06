<script lang="ts">
  import { readerSource } from "$lib/domain/inspectOutline";
  import { categorizeRow, categoryMeta, inspectBody, type InspectRow } from "$lib/promptInspectView";
  import { Button, SegmentedChoice } from "$lib/ui";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import Markdown from "../chat/Markdown.svelte";

  /** One part of the request: formatted to read, or the exact text that was sent. */
  type Props = { row: InspectRow | undefined; totalTokens: number };
  let { row, totalTokens }: Props = $props();

  const MODES = [
    { value: "reader", label: "Reader", description: "Formatted to read" },
    { value: "raw", label: "Raw", description: "The exact text sent, with line numbers" },
  ] as const;
  let mode = $state<"reader" | "raw">("reader");
  let wrap = $state(true);
  const copied = copyFeedback();

  const category = $derived(row ? categorizeRow(row) : null);
  const meta = $derived(category ? categoryMeta(category) : null);
  const title = $derived(row ? (row.kind === "msg" ? row.part.label : row.tool.name) : "");
  const tokens = $derived(row ? (row.kind === "msg" ? row.part.tokens : row.tool.tokens) : 0);
  const body = $derived(row ? inspectBody(row) : "");
  const lines = $derived(body.split("\n"));
  const share = $derived(totalTokens > 0 ? Math.round((tokens / totalTokens) * 100) : 0);
</script>

{#if row && meta}
  <article class="inspector-reader">
    <header class="inspector-reader-head">
      <div class="inspector-reader-title">
        <span class="inspector-kind" style:--kind={meta.color}>{meta.label}</span>
        <h2>{title}</h2>
        <span class="inspector-reader-tokens">{fmtTokens(tokens)} tokens{share ? ` · ${share}% of the request` : ""}</span>
      </div>
      <div class="inspector-reader-actions">
        {#if mode === "raw"}
          <Button size="s" class="inspector-wrap" aria-pressed={wrap} onclick={() => (wrap = !wrap)}>Wrap lines</Button>
        {/if}
        <SegmentedChoice compact label="View" options={MODES} value={mode} onSelect={(m) => (mode = m)} />
        <Button variant="icon" aria-label="Copy this part" title="Copy this part" disabled={!body} onclick={() => void copied.copy(body)}>
          <Icon icon={copied.copied ? Check : Copy} size={14} />
        </Button>
      </div>
    </header>
    <p class="inspector-reader-desc">{meta.desc}</p>

    {#if !body.trim()}
      <p class="inspector-empty">This part is empty.</p>
    {:else if mode === "reader"}
      <div class="inspector-doc"><Markdown text={readerSource(row)} /></div>
    {:else}
      <div class="inspector-raw" class:is-wrapped={wrap} role="table" aria-label="Raw text">
        {#each lines as line, i (i)}
          <div class="inspector-raw-line" role="row">
            <span class="inspector-raw-num">{i + 1}</span>
            <span class="inspector-raw-text">{line || " "}</span>
          </div>
        {/each}
      </div>
    {/if}
  </article>
{:else}
  <p class="inspector-empty">Pick a part of the request on the left.</p>
{/if}
