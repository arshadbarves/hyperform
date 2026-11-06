<script lang="ts">
  import { setTrust } from "$lib/commands";
  import type { SetupChecklist, SetupItemId } from "$lib/domain/onboarding";
  import { startersFor } from "$lib/domain/starters";
  import { errorText, pushToast } from "$lib/runtime";
  import { composer } from "$lib/stores/composer.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { openSettings } from "$lib/stores/app-actions";
  import { Button, ProgressRing } from "$lib/ui";
  import Icon, { Check } from "$lib/ui/icons";

  /** What this project still needs, with a one-click fix each; hidden once everything is done. */
  type Props = { list: SetupChecklist; root: string };
  let { list, root }: Props = $props();

  const ACTION: Record<SetupItemId, string> = { model: "Connect", trust: "Trust", instructions: "Write it for me" };

  async function fix(id: SetupItemId) {
    if (id === "model") return openSettings("providers");
    if (id === "instructions") {
      const prompt = startersFor({ changed: 0, hasInstructions: false }).top[0].prompt;
      return composer.setDraft(prompt);
    }
    try {
      await setTrust(root, true);
      await settingsStore.loadTrust();
    } catch (e) {
      pushToast(`Could not trust the project: ${errorText(e)}`, "warn");
    }
  }
</script>

<section class="home-card setup-card" aria-label="Set up this project">
  <header class="home-card-head">
    <h2 class="home-card-title">Set up this project</h2>
    <span class="setup-progress">
      <ProgressRing value={list.done / list.total} size={14} stroke={2} tone="ok" />
      {list.done} of {list.total} done
    </span>
  </header>
  <ul class="setup-list">
    {#each list.items as item (item.id)}
      <li class:is-done={item.done}>
        <span class="setup-check" aria-hidden="true">{#if item.done}<Icon icon={Check} size={11} strokeWidth={2.4} />{/if}</span>
        <span class="setup-text">
          <span class="setup-label">{item.label}</span>
          {#if !item.done}<span class="setup-hint">{item.hint}</span>{/if}
        </span>
        {#if !item.done}
          <Button variant="secondary" size="s" onclick={() => void fix(item.id)}>{ACTION[item.id]}</Button>
        {/if}
      </li>
    {/each}
  </ul>
</section>
