<script lang="ts">
  import type { PromptInspect } from "$lib/domain/requestInspect";
  import { promptInsights } from "$lib/promptInsights";
  import { Disclosure } from "$lib/ui";
  import { fmtTokens } from "$lib/util";

  /** Where the budget goes and what would shrink it; folded until asked for. */
  type Props = { snap: PromptInspect };
  let { snap }: Props = $props();

  let open = $state(false);
  const insights = $derived(promptInsights(snap));
</script>

<Disclosure bind:open class="inspector-insights" summaryClass="inspector-insights-summary">
  {#snippet summary()}
    <span>Insights</span>
    <span class="inspector-insights-hint">Largest: {insights.largest.name} · {Math.round(insights.largest.share * 100)}%</span>
  {/snippet}
  <dl class="inspector-facts">
    <div>
      <dt>Reusable across turns</dt>
      <dd>{fmtTokens(insights.cacheableTokens)} tokens (system prompt and tools, which the provider can cache)</dd>
    </div>
    <div>
      <dt>Changes every turn</dt>
      <dd>{fmtTokens(insights.volatileTokens)} tokens, ending with {insights.volatileTail}</dd>
    </div>
  </dl>
  {#if insights.hints.length}
    <ul class="inspector-hints">
      {#each insights.hints as hint (hint)}<li>{hint}</li>{/each}
    </ul>
  {/if}
</Disclosure>
