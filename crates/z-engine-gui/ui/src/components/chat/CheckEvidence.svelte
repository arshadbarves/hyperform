<script lang="ts">
  import { checkDetail, checkStatus } from "$lib/domain/verification";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";

  type Props = { records: CheckRecord[] };
  let { records }: Props = $props();
</script>

{#if records.length === 0}
  <p class="check-empty">No checks ran during this turn.</p>
{:else}
  <div class="check-list">
    {#each records as record (record.recordId)}
      {@const status = checkStatus(record)}
      <details class={`check-row tone-${status.tone}`}>
        <summary>
          <span class="check-dot" aria-hidden="true"></span>
          <span class="check-label">{record.label}</span>
          <span class="check-kind">{record.kind}</span>
          <span class="check-status">{status.label}</span>
          <span class="check-detail">{checkDetail(record)}</span>
        </summary>
        <pre class="check-command">$ {record.command}</pre>
        {#if record.outputTail}<pre class="check-output">{record.outputTail}</pre>{/if}
        {#if record.artifact}<p class="check-artifact">Full output · <code>{record.artifact}</code></p>{/if}
      </details>
    {/each}
  </div>
{/if}
