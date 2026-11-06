<script lang="ts">
  import type { RouteView } from "$lib/domain/sessionView";
  import { routeChip } from "$lib/domain/timeline/routeChip";
  import Icon, { Zap } from "$lib/ui/icons";

  /** The effort or model a task was routed to (`decisions_routing`), and why. */
  type Props = { route: RouteView };
  let { route }: Props = $props();

  const chip = $derived(routeChip(route.route));
</script>

<div class="route-chip" role="note">
  <Icon icon={Zap} size={11} class="route-chip-icon" />
  <span class="route-chip-lead">{chip.lead}</span>
  <span class="route-chip-choice">{chip.choice}</span>
  {#if chip.reason}<span class="route-chip-reason">{chip.reason}</span>{/if}
</div>
