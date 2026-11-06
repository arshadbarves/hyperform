<script lang="ts">
  import { modLabel } from "$lib/platform";
  import Icon, { ArrowUp, CornerDownLeft, Square, Terminal } from "$lib/ui/icons";
  import ComposerPlusMenu from "./ComposerPlusMenu.svelte";
  import ModePicker from "./ModePicker.svelte";
  import ModelPicker from "./ModelPicker.svelte";
  import PlanChip from "./PlanChip.svelte";

  /**
   * Under the draft: the + menu, the Plan chip (while planning) and the
   * permission level on the left; the model and Send on the right. While a turn runs, Send becomes Stop and a typed message is
   * queued (Enter) or interrupts (⌘Enter).
   */
  type Props = {
    shellMode: boolean;
    busy: boolean;
    canSend: boolean;
    hasText: boolean;
    showTerminal: boolean;
    onAttach: () => void;
    onInsert: (prefix: string) => void;
    onShowShell: () => void;
    onSend: () => void;
    onInterrupt: () => void;
    onCancel: () => void;
  };

  let { shellMode, busy, canSend, hasText, showTerminal, onAttach, onInsert, onShowShell, onSend, onInterrupt, onCancel }: Props =
    $props();

  const mod = modLabel();
</script>

<div class="composer-bar">
  {#if shellMode}
    <div class="composer-controls-left">
      <span class="shell-mode-pill">
        <Icon icon={Terminal} size={11} />
        <span>Shell</span>
      </span>
      <span class="composer-hint">Runs in the project, without the agent · <kbd class="kbd">Esc</kbd> to leave</span>
    </div>
  {:else}
    <div class="composer-controls-left">
      <ComposerPlusMenu {showTerminal} {onAttach} {onInsert} onShowTerminal={onShowShell} />
      <PlanChip />
      <ModePicker />
    </div>
  {/if}

  <div class="composer-actions-right">
    {#if busy && hasText && !shellMode}
      <span class="composer-hint">
        Enter queues ·
        <button type="button" class="composer-hint-btn" onclick={onInterrupt}>{mod}Enter interrupts</button>
      </span>
    {/if}
    {#if shellMode}
      <button type="button" class="composer-send is-run" title="Run (Enter)" onclick={onSend} disabled={!canSend}>
        <Icon icon={CornerDownLeft} size={12} />
        <span>Run</span>
      </button>
    {:else}
      <ModelPicker />
      {#if busy && hasText}
        <button type="button" class="composer-icon-btn" title="Stop the turn (Esc)" aria-label="Stop the turn" onclick={onCancel}>
          <Icon icon={Square} size={10} />
        </button>
      {/if}
      {@const stop = busy && !hasText}
      <button
        type="button"
        class={`composer-send${stop ? " is-stop" : ""}`}
        title={stop ? "Stop the turn (Esc)" : busy ? "Queue for the agent (Enter)" : "Send (Enter)"}
        aria-label={stop ? "Stop the turn" : busy ? "Queue message" : "Send"}
        onclick={stop ? onCancel : onSend}
        disabled={!stop && !canSend}
      >
        {#key stop}
          <span class="composer-send-glyph">
            {#if stop}<Icon icon={Square} size={11} />{:else}<Icon icon={ArrowUp} size={15} strokeWidth={2.4} />{/if}
          </span>
        {/key}
      </button>
    {/if}
  </div>
</div>
