<script lang="ts">
  import type { Snippet } from "svelte";
  import { groupOfSetting } from "$lib/domain/settings/searchIndex";
  import { ui } from "$lib/stores/ui.svelte";
  import { Disclosure } from "$lib/ui";
  import { groupsFold } from "./folding";

  /**
   * A titled block of settings. A folding group shows only its title and
   * line until opened, and opens by itself when search points inside it.
   */
  type Props = {
    title: string;
    description: string;
    /** Fold this group; groups under `foldGroups()` fold without it. */
    collapsible?: boolean;
    children: Snippet;
  };

  let { title, description, collapsible, children }: Props = $props();

  const inherited = groupsFold();
  const folds = $derived(collapsible ?? inherited);
  let open = $state(false);

  $effect(() => {
    if (folds && groupOfSetting(ui.settingsFocus) === title) open = true;
  });
</script>

{#if folds}
  <section class="settings-group is-folding" data-setting={`@${title}`}>
    <Disclosure bind:open summaryClass="settings-group-summary">
      {#snippet summary()}
        <span class="settings-group-head">
          <span class="settings-group-title">{title}</span>
          <span class="settings-group-sub">{description}</span>
        </span>
      {/snippet}
      <div class="settings-group-body">{@render children()}</div>
    </Disclosure>
  </section>
{:else}
  <section class="settings-group" data-setting={`@${title}`}>
    <div class="settings-group-head">
      <h3 class="settings-group-title">{title}</h3>
      <span class="settings-group-sub">{description}</span>
    </div>
    {@render children()}
  </section>
{/if}
