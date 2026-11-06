<script lang="ts">
  import type { SessionView } from "$lib/domain/sessionView";
  import { currentSuggestions } from "$lib/domain/sessionView/suggestions";
  import PlanSuggestion from "./PlanSuggestion.svelte";
  import ReviewSuggestion from "./ReviewSuggestion.svelte";
  import RuleSuggestion from "./RuleSuggestion.svelte";

  /** Cards decision uses offered for the current turn; each acts only on a click. */
  type Props = { view: SessionView };
  let { view }: Props = $props();

  const shown = $derived(currentSuggestions(view));
  const projectRoot = $derived(view.info?.projectRoot ?? null);
  const busy = $derived(view.status !== "idle");
</script>

{#each shown as suggestion (suggestion.suggestionId)}
  {#if suggestion.kind.type === "planFirst"}
    <PlanSuggestion suggestionId={suggestion.suggestionId} />
  {:else if suggestion.kind.type === "saveRule"}
    <RuleSuggestion suggestionId={suggestion.suggestionId} rule={suggestion.kind.rule} {projectRoot} />
  {:else}
    <ReviewSuggestion
      suggestionId={suggestion.suggestionId}
      areas={suggestion.kind.areas}
      paths={suggestion.kind.paths}
      {busy}
    />
  {/if}
{/each}
