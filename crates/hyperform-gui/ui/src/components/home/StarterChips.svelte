<script lang="ts">
  import type { Starter } from "$lib/domain/starters";
  import { composer } from "$lib/stores/composer.svelte";
  import Icon, { ChevronDown } from "$lib/ui/icons";
  import StarterIcon from "./StarterIcon.svelte";

  /** Three ready-made first prompts that fit the project; the rest wait behind "More ideas". */
  type Props = { top: Starter[]; more: Starter[] };
  let { top, more }: Props = $props();

  let showMore = $state(false);
  const shown = $derived(showMore ? [...top, ...more] : top);
</script>

<div class="starter-chips">
  {#each shown as starter (starter.id)}
    <button type="button" class="starter-chip" title={starter.prompt} onclick={() => composer.setDraft(starter.prompt)}>
      <StarterIcon kind={starter.kind} />
      <span>{starter.title}</span>
    </button>
  {/each}
  {#if more.length && !showMore}
    <button type="button" class="starter-more" onclick={() => (showMore = true)}>
      <span>More ideas</span>
      <Icon icon={ChevronDown} size={11} strokeWidth={2} />
    </button>
  {/if}
</div>
