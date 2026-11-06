<script lang="ts">
  import { MODES, modeMeta, type ModeMeta } from "$lib/domain/modes";
  import type { PermissionMode } from "$lib/protocol/PermissionMode";
  import { sessions, setMode } from "$lib/runtime";
  import { Popover } from "$lib/ui";
  import Icon, { AlertTriangle, Check, ChevronDown, Shield, ShieldAlert } from "$lib/ui/icons";

  /** How much the agent may do without asking. Plan mode is its own chip (`PlanChip`). */
  const mode: PermissionMode = $derived(sessions.active?.mode ?? "default");
  const current = $derived(modeMeta(mode));
  let open = $state(false);
  let confirming = $state<ModeMeta | null>(null);

  function pick(meta: ModeMeta) {
    if (meta.warning && confirming?.id !== meta.id) {
      confirming = meta;
      return;
    }
    open = false;
    confirming = null;
    if (meta.id !== mode) void setMode(meta.id);
  }
</script>

<Popover.Root
  bind:open
  onOpenChange={(next) => {
    if (!next) confirming = null;
  }}
>
  <Popover.Trigger
    class={`composer-chip${mode === "bypass" ? " is-danger" : ""}`}
    title="Permission mode"
  >
    <Icon icon={mode === "bypass" ? ShieldAlert : Shield} size={12} />
    <span>{current.label}</span>
    <Icon icon={ChevronDown} size={10} strokeWidth={2.2} />
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content class="chip-pop glass-strong" side="top" align="start" sideOffset={8}>
      <p class="chip-pop-head">Permission mode</p>
      {#if confirming}
        <div class="mode-warning" role="alert">
          <Icon icon={AlertTriangle} size={14} />
          <span>{confirming.warning}</span>
        </div>
        <div class="mode-warning-actions">
          <button type="button" class="btn-danger" onclick={() => confirming && pick(confirming)}>Turn on {confirming.label}</button>
          <button type="button" class="btn-ghost" onclick={() => (confirming = null)}>Back</button>
        </div>
      {:else}
        {#each MODES as meta (meta.id)}
          <button type="button" class={`chip-pop-row${meta.warning ? " is-danger" : ""}`} onclick={() => pick(meta)}>
            <span class="chip-pop-text">
              <span class="chip-pop-label">{meta.label}</span>
              <span class="chip-pop-sub">{meta.description}</span>
            </span>
            {#if meta.id === mode}<Icon icon={Check} size={13} strokeWidth={2.2} />{/if}
          </button>
        {/each}
      {/if}
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
