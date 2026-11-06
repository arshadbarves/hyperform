<script lang="ts">
  import { parseGitDiff } from "$lib/diffParse";
  import { foldContext, langForPath, splitRows, type DiffDisplayRow } from "$lib/domain/diffRows";
  import { plural } from "$lib/domain/format";
  import type { DiffLayout } from "$lib/stores/ui.svelte";
  import DiffCode from "./DiffCode.svelte";

  /**
   * The rows of one file's diff, syntax-colored. Long unchanged stretches
   * fold to a line you can open; very long diffs show the first rows and
   * offer the rest. Used by the Changes panel and inline in tool cards.
   */
  type Props = { text: string; path: string; layout?: DiffLayout; maxRows?: number };
  let { text, path, layout = "unified", maxRows = 400 }: Props = $props();

  let opened = $state(new Set<number>());
  let showAll = $state(false);

  const diff = $derived(parseGitDiff(text));
  const lang = $derived(langForPath(diff.path ?? path));
  const rows = $derived(foldContext(diff.rows, opened));
  const shown = $derived(showAll ? rows : rows.slice(0, maxRows));
  const hiddenRows = $derived(rows.length - shown.length);
  const split = $derived(layout === "split" ? splitRows(shown) : []);

  function unfold(from: number) {
    opened = new Set([...opened, from]);
  }

  function rowKey(row: DiffDisplayRow, i: number): string {
    return row.kind === "fold" ? `f${row.from}` : `${i}:${row.oldNo ?? ""}:${row.newNo ?? ""}`;
  }

  const SIGN = { add: "+", del: "−", ctx: " " } as const;
</script>

{#if diff.rows.length === 0}
  <p class="diff-view-empty">{text.trim() ? text : "No line changes to show (the file may be binary or only its mode changed)."}</p>
{:else if layout === "split"}
  <div class="diff-table is-split" role="table" aria-label={`Changes in ${path}`}>
    {#each split as row (row.key)}
      {#if row.kind === "hunk"}
        <div class="diff-hunk" role="row">⋯ line {row.row.newNo ?? row.row.oldNo ?? ""}</div>
      {:else if row.kind === "fold"}
        <button type="button" class="diff-fold" onclick={() => unfold(row.row.from)}>Show {plural(row.row.count, "unchanged line")}</button>
      {:else}
        <div class="diff-split-row" role="row">
          <span class={`diff-num ${row.left?.kind ?? "empty"}`}>{row.left?.oldNo ?? ""}</span>
          <span class={`diff-side ${row.left?.kind === "ctx" ? "ctx" : row.left ? "del" : "empty"}`}>
            {#if row.left}<DiffCode text={row.left.text} {lang} />{/if}
          </span>
          <span class={`diff-num ${row.right?.kind ?? "empty"}`}>{row.right?.newNo ?? ""}</span>
          <span class={`diff-side ${row.right?.kind === "ctx" ? "ctx" : row.right ? "add" : "empty"}`}>
            {#if row.right}<DiffCode text={row.right.text} {lang} />{/if}
          </span>
        </div>
      {/if}
    {/each}
  </div>
{:else}
  <div class="diff-table" role="table" aria-label={`Changes in ${path}`}>
    {#each shown as row, i (rowKey(row, i))}
      {#if row.kind === "fold"}
        <button type="button" class="diff-fold" onclick={() => unfold(row.from)}>Show {plural(row.count, "unchanged line")}</button>
      {:else if row.kind === "hunk"}
        <div class="diff-hunk" role="row">⋯ line {row.newNo ?? row.oldNo ?? ""}</div>
      {:else}
        <div class={`diff-line ${row.kind}`} role="row">
          <span class="diff-num">{row.oldNo ?? ""}</span>
          <span class="diff-num">{row.newNo ?? ""}</span>
          <span class="diff-sign" aria-hidden="true">{SIGN[row.kind]}</span>
          <DiffCode text={row.text} {lang} />
        </div>
      {/if}
    {/each}
  </div>
{/if}

{#if hiddenRows > 0}
  <button type="button" class="btn-secondary diff-more" onclick={() => (showAll = true)}>Show the remaining {plural(hiddenRows, "line")}</button>
{/if}
