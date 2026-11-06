<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { chatChanges, sessions } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { GitCompare } from "$lib/ui/icons";
  import TitlebarButton from "./TitlebarButton.svelte";

  /** Opens the diff review; the count is how many files this chat has changed. */
  const mod = modLabel();
  const view = $derived(sessions.active);
  const turnsDone = $derived(view?.turns.length ?? 0);
  const count = $derived(chatChanges.count(sessions.activeId));
  const label = $derived(count ? `Review changes · ${count} file${count === 1 ? "" : "s"} changed in this chat` : "Review changes");

  $effect(() => {
    const id = sessions.activeId;
    void turnsDone;
    if (id && view?.info) void chatChanges.refresh(id);
  });
</script>

{#snippet counted()}
  {#key count}<span class="changes-count">{count}</span>{/key}
{/snippet}

<TitlebarButton
  {label}
  shortcut={`${mod}D`}
  icon={GitCompare}
  pressed={ui.panelTab === "changes"}
  extra={count ? counted : undefined}
  onclick={() => ui.togglePanel("changes")}
/>
