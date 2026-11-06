<script lang="ts">
  import { sessionDecisions, type SessionDecisions } from "$lib/commands";
  import { decisionFacts, decisionLine, decisionsHint, featureTitles } from "$lib/domain/decisionTrace";
  import { sessions } from "$lib/runtime";
  import { Disclosure } from "$lib/ui";

  /**
   * What the running experimental decision features asked and what the
   * engine did; shown only while a feature runs or decisions were recorded.
   */
  const LIMIT = 50;

  let open = $state(false);
  let data = $state<SessionDecisions | null>(null);

  const facts = $derived(data ? decisionFacts(data) : []);
  const hint = $derived(data ? decisionsHint(data) : "");
  const lines = $derived.by(() => {
    if (!data) return [];
    const titles = featureTitles(data);
    return data.records.map((record) => decisionLine(record, titles));
  });
  const shown = $derived(data !== null && (data.features.length > 0 || data.records.length > 0));

  // Fresh after each turn (busy -> idle) and whenever the section opens.
  $effect(() => {
    const id = sessions.activeId;
    const status = sessions.active?.status;
    void open;
    if (!id || status === "busy") return;
    let stale = false;
    sessionDecisions(id, LIMIT)
      .then((next) => {
        if (!stale) data = next;
      })
      .catch((e: unknown) => console.warn("session_decisions unavailable", e));
    return () => {
      stale = true;
    };
  });
</script>

{#if shown}
  <Disclosure bind:open class="inspector-insights inspector-decisions" summaryClass="inspector-insights-summary">
    {#snippet summary()}
      <span>Decisions</span>
      <span class="inspector-insights-hint">{hint}</span>
    {/snippet}
    <dl class="inspector-facts">
      {#each facts as fact (fact.label)}
        <div>
          <dt>{fact.label}</dt>
          <dd>{fact.value}</dd>
        </div>
      {/each}
    </dl>
    {#if lines.length}
      <ol class="decision-records">
        {#each lines as line (line.seq)}
          <li class={`decision-record tone-${line.tone}`}>
            <span class="decision-record-title">{line.title}</span>
            <span class="decision-record-detail">{line.detail}</span>
            {#if line.why}<span class="decision-record-why">Overridden: {line.why}</span>{/if}
          </li>
        {/each}
      </ol>
    {:else}
      <p class="decision-records-empty">No decisions yet in this chat.</p>
    {/if}
  </Disclosure>
{/if}
