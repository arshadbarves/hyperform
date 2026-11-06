<script lang="ts">
  import type { IslandAction } from "$lib/domain/island";
  import type { LiveTone } from "$lib/domain/liveStatus";
  import type { PetPose } from "$lib/domain/pet/pose";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import IslandPet from "../pet/IslandPet.svelte";

  /**
   * The island's capsule row: the pet, one line, its clock and, when this chat
   * needs you, one button. Plan progress fills the capsule itself; compacting
   * sweeps it. Opened, the row stays on as the card's header.
   */
  type Props = {
    tone: LiveTone;
    text: string | null;
    metric: string | null;
    pose: PetPose | null;
    progress: number | null;
    helpers: number;
    sweeping: boolean;
    unseen: boolean;
    action: IslandAction | null;
    expanded: boolean;
    /** The card's id while it is there. */
    controls: string | undefined;
    label: string;
    toggle?: HTMLButtonElement;
    onToggle: () => void;
    onAction: () => void;
  };
  let {
    tone,
    text,
    metric,
    pose,
    progress,
    helpers,
    sweeping,
    unseen,
    action,
    expanded,
    controls,
    label,
    toggle = $bindable(),
    onToggle,
    onAction,
  }: Props = $props();
</script>

<div class="island-row">
  {#if progress !== null}
    <span class="island-fill" style:scale={`${progress} 1`} aria-hidden="true"></span>
  {:else if sweeping}
    <span class="island-fill is-sweep" aria-hidden="true"></span>
  {/if}
  <button
    bind:this={toggle}
    type="button"
    class="island-toggle"
    aria-label={label}
    aria-expanded={expanded}
    aria-controls={controls}
    onclick={onToggle}
  >
    <span
      class="island-pet-slot"
      role="presentation"
      data-splash-target
      onpointerenter={() => (userSignals.hovering = true)}
      onpointerleave={() => (userSignals.hovering = false)}
    >
      {#if pose}
        <IslandPet {pose} {progress} {helpers} />
      {:else}
        <span class={`island-dot tone-${tone}`}></span>
      {/if}
    </span>
    {#if text}{#key text}<span class="island-text">{text}</span>{/key}{/if}
    {#if metric}<span class="island-metric">{metric}</span>{/if}
    {#if unseen}<span class="island-unseen" aria-hidden="true"></span>{/if}
  </button>
  {#if action}
    <button type="button" class="island-action" aria-label={action.hint} onclick={onAction}>{action.label}</button>
  {/if}
</div>
