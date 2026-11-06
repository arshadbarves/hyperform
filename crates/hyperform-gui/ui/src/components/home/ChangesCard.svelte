<script lang="ts">
  import type { ChangedFile } from "$lib/commands";
  import { ui } from "$lib/stores/ui.svelte";

  /** Uncommitted work in the project: the first few files, then the full review. */
  type Props = { files: ChangedFile[] };
  let { files }: Props = $props();

  const LETTER: Record<string, string> = { added: "A", modified: "M", deleted: "D" };
  const shown = $derived(files.slice(0, 3));
</script>

<section class="home-card" aria-label="Uncommitted changes">
  <header class="home-card-head">
    <h2 class="home-card-title">Uncommitted changes</h2>
    <button type="button" class="home-card-link" onclick={() => ui.openPanel("changes", null, "git")}>Review all {files.length}</button>
  </header>
  <ul class="changes-list">
    {#each shown as file (file.path)}
      <li>
        <button type="button" class="changes-row" onclick={() => ui.openPanel("changes", file.path, "git")}>
          <span class={`changes-letter is-${file.status}`}>{LETTER[file.status] ?? "M"}</span>
          <span class="changes-path" title={file.path}>{file.path}</span>
          <span class="changes-counts">
            {#if file.added}<span class="is-add">+{file.added}</span>{/if}
            {#if file.deleted}<span class="is-del">−{file.deleted}</span>{/if}
          </span>
        </button>
      </li>
    {/each}
  </ul>
</section>
