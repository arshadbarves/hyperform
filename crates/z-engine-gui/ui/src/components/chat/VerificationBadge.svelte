<script lang="ts">
  import { untrack } from "svelte";
  import { plural } from "$lib/domain/format";
  import { verificationBadge } from "$lib/domain/verification";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";
  import type { VerificationOutcome } from "$lib/protocol/VerificationOutcome";
  import Icon, { AlertOctagon, AlertTriangle, CheckCircle2, Info } from "$lib/ui/icons";
  import CheckEvidence from "./CheckEvidence.svelte";

  type Props = { outcome: VerificationOutcome; evidence: CheckRecord[]; startOpen?: boolean };
  let { outcome, evidence, startOpen = false }: Props = $props();

  const ICONS = { ok: CheckCircle2, warn: AlertTriangle, err: AlertOctagon, neutral: Info } as const;
  const badge = $derived(verificationBadge(outcome));
  const hasDetail = $derived(evidence.length > 0 || Boolean(badge.reason));
  let open = $state(untrack(() => startOpen));
</script>

<div class="verify-wrap">
  <button
    type="button"
    class={`verify-badge tone-${badge.tone}`}
    disabled={!hasDetail}
    aria-expanded={hasDetail ? open : undefined}
    onclick={() => (open = !open)}
  >
    <Icon icon={ICONS[badge.tone]} size={11} />
    <span>{badge.label}</span>
    {#if evidence.length > 0}<span class="verify-count">{plural(evidence.length, "check")}</span>{/if}
  </button>
  {#if open && hasDetail}
    <div class="verify-detail">
      {#if badge.reason}<p class="verify-reason">{badge.reason}</p>{/if}
      <CheckEvidence records={evidence} />
    </div>
  {/if}
</div>
