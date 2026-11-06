<script lang="ts">
  import type { CtxMeter } from "$lib/domain/contextMeter";
  import { contextBubble } from "$lib/domain/island";
  import { island } from "$lib/stores/island.svelte";
  import { Popover, ProgressRing, Tooltip } from "$lib/ui";
  import { fmtTokens } from "$lib/util";
  import ContextCard from "./ContextCard.svelte";

  /** How full this chat's context is; the number appears once it matters. */
  type Props = { sessionId: string; meter: CtxMeter; busy: boolean };
  let { sessionId, meter, busy }: Props = $props();

  const look = $derived(contextBubble(meter));
  const text = $derived(`Context ${meter.pct}% · ${fmtTokens(meter.used)} of ${fmtTokens(meter.max)}`);
</script>

<Popover.Root bind:open={island.contextOpen}>
  <Tooltip {text}>
    <Popover.Trigger class={`satellite context-bubble tone-${look.tone}`} aria-label={text}>
      <ProgressRing value={meter.pct / 100} size={16} stroke={2.2} tone={look.tone} />
      {#if look.label}<span class="satellite-label">{look.label}</span>{/if}
    </Popover.Trigger>
  </Tooltip>
  <Popover.Portal>
    <Popover.Content class="context-card" side="bottom" align="end" sideOffset={8} collisionPadding={12}>
      <ContextCard {sessionId} {meter} {busy} onClose={() => (island.contextOpen = false)} />
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
