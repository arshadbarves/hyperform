<script lang="ts">
  import { parseGitDiff } from "$lib/diffParse";
  import type { DiffLayout } from "$lib/stores/ui.svelte";
  import DiffFileBar from "./DiffFileBar.svelte";
  import DiffRows from "./DiffRows.svelte";

  /** One file in the Changes panel: its file bar with actions, then its rows. */
  type Props = { text: string; path: string; root: string | null; deletedFile: boolean; layout: DiffLayout };
  let { text, path, root, deletedFile, layout }: Props = $props();

  const diff = $derived(parseGitDiff(text));
</script>

<div class="diff-view">
  <DiffFileBar {path} {root} added={diff.added} deleted={diff.deleted} {deletedFile} diff={text} />
  <DiffRows {text} {path} {layout} />
</div>
