<script lang="ts">
  import type { AgentUsageRow } from "$lib/domain/agentTree";
  import { promptTokens } from "$lib/domain/usage";
  import { fmtCost, fmtTokens } from "$lib/util";

  type Props = { rows: AgentUsageRow[] };
  let { rows }: Props = $props();
</script>

<section class="work-section">
  <h4 class="work-section-title">Usage by agent</h4>
  <div class="usage-table" role="table">
    <div class="usage-row usage-head" role="row">
      <span role="columnheader">Agent</span>
      <span role="columnheader">In</span>
      <span role="columnheader">Out</span>
      <span role="columnheader">Cost</span>
    </div>
    {#each rows as row (row.agentId)}
      <div class="usage-row" role="row">
        <span class="usage-label" role="cell" title={row.label}>{row.label}</span>
        <span role="cell">{fmtTokens(promptTokens(row.usage))}</span>
        <span role="cell">{fmtTokens(row.usage.outputTokens)}</span>
        <span role="cell">{row.costUsd === null ? "–" : fmtCost(row.costUsd)}</span>
      </div>
    {/each}
  </div>
</section>
