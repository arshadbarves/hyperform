<script lang="ts">
  import { fmtDuration } from "$lib/domain/format";
  import { receiptPlan } from "$lib/domain/receipt";
  import { baseName } from "$lib/domain/tools/toolInput";
  import { usageLine } from "$lib/domain/usage";
  import { claimNote, outcomeNote, turnEvidence } from "$lib/domain/verification";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";
  import type { TurnRecord } from "$lib/protocol/TurnRecord";
  import { sessions } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { fmtCost } from "$lib/util";
  import VerificationBadge from "./VerificationBadge.svelte";

  /**
   * The turn's receipt: one quiet line whose detail follows
   * `ui.task_report_view`. Hovering it always gives time, tokens and cost.
   */
  type Props = { record: TurnRecord; checks: CheckRecord[]; files: string[] };
  let { record, checks, files }: Props = $props();

  const FILES_SHOWN = 4;
  const plan = $derived(receiptPlan(settingsStore.settings?.ui.task_report_view ?? "quiet", record));
  const note = $derived(outcomeNote(record.outcome));
  const claim = $derived(claimNote(sessions.active?.claims[record.turnId], record.verification));
  const evidence = $derived(turnEvidence(record, checks));
  const duration = $derived(fmtDuration(record.finishedAt - record.startedAt));
  const usage = $derived(usageLine(record.usage));
  const cost = $derived(record.costUsd > 0 ? fmtCost(record.costUsd) : "");
  const stats = $derived([plan.stats && duration, plan.usage && usage, plan.stats && cost].filter(Boolean));
  const hint = $derived([duration, usage, cost].filter(Boolean).join(" · "));
  const showFiles = $derived(plan.files && files.length > 0);
</script>

{#if plan.badge || note || claim || stats.length || showFiles}
  <footer class="turn-receipt" title={hint || undefined}>
    {#if plan.badge}<VerificationBadge outcome={record.verification} {evidence} startOpen={plan.checksOpen} />{/if}
    {#if claim}<span class="turn-outcome tone-warn" title={claim.hint}>{claim.label}</span>{/if}
    {#if note}<span class={`turn-outcome tone-${note.tone}`}>{note.label}</span>{/if}
    {#if showFiles}
      <span class="receipt-files" aria-label="Files changed in this turn">
        {#each files.slice(0, FILES_SHOWN) as file (file)}
          <button type="button" class="receipt-file" title={`Review ${file}`} onclick={() => ui.openPanel("changes", file, "session")}>
            {baseName(file)}
          </button>
        {/each}
        {#if files.length > FILES_SHOWN}
          <button type="button" class="receipt-file is-more" onclick={() => ui.openPanel("changes", null, "session")}>
            +{files.length - FILES_SHOWN}
          </button>
        {/if}
      </span>
    {/if}
    {#if stats.length > 0}<span class="turn-stats">{stats.join(" · ")}</span>{/if}
  </footer>
{/if}
