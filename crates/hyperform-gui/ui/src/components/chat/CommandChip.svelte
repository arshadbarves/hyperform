<script lang="ts">
  import type { CommandChip } from "$lib/domain/timeline/commandChip";
  import Icon, { ChevronRight, Sparkles } from "$lib/ui/icons";

  type Props = { chip: CommandChip };
  let { chip }: Props = $props();

  let open = $state(false);
</script>

<div class="command-chip">
  <button
    type="button"
    class="command-chip-head"
    aria-expanded={open}
    title={open ? "Hide the prompt this command sent" : "Show the prompt this command sent"}
    onclick={() => (open = !open)}
  >
    <Icon icon={Sparkles} size={11} strokeWidth={1.8} />
    <span class="command-chip-name">/{chip.name}</span>
    {#if chip.args}<span class="command-chip-args">{chip.args}</span>{/if}
    <span class={`command-chip-caret${open ? " open" : ""}`}><Icon icon={ChevronRight} size={11} /></span>
  </button>
  {#if open}
    <pre class="command-chip-body">{chip.body}</pre>
  {/if}
</div>

<style>
  .command-chip {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .command-chip-head {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    max-width: 100%;
    padding: 3px 8px;
    border: 1px solid var(--separator);
    border-radius: 6px;
    background: var(--fill-quiet);
    color: var(--label-2);
    font-family: var(--font-mono);
    font-size: 12px;
    cursor: pointer;
  }

  .command-chip-head:hover {
    color: inherit;
  }

  .command-chip-name {
    font-weight: 600;
    color: var(--accent);
  }

  .command-chip-args {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .command-chip-caret {
    display: inline-flex;
    transition: transform 0.15s ease;
  }

  .command-chip-caret.open {
    transform: rotate(90deg);
  }

  .command-chip-body {
    margin: 0;
    max-height: 320px;
    overflow: auto;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--fill-quiet);
    color: var(--label-2);
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
