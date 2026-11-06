<script lang="ts" generics="T extends string">
  import { nextSegmentIndex } from "$lib/domain/segmentedChoice";

  type SegmentOption<T extends string> = {
    value: T;
    label: string;
    description: string;
  };

  /** Exclusive choices side by side; a thumb slides to the chosen one. */
  type Props = {
    label: string;
    options: readonly SegmentOption<T>[];
    value: T;
    /** Persisting a choice must not disable the controls: a disabled button
     *  loses focus, which drops a keyboard reader out of the group. */
    busy?: boolean;
    /** Toolbar size: for panel headers and filters rather than forms. */
    compact?: boolean;
    onSelect: (value: T) => void;
  };

  let { label, options, value, busy = false, compact = false, onSelect }: Props = $props();
  let buttons = $state<HTMLButtonElement[]>([]);
  const selected = $derived(options.findIndex((o) => o.value === value));

  function activate(index: number) {
    const option = options[index];
    if (!option) return;
    // WebKit does not focus a button on click, so the group would otherwise
    // keep arrow-key focus on the previously checked option.
    buttons[index]?.focus();
    if (option.value === value) return;
    onSelect(option.value);
  }

  function moveSelection(event: KeyboardEvent, index: number) {
    const nextIndex = nextSegmentIndex(event.key, index, options.length);
    if (nextIndex === null) return;
    event.preventDefault();
    activate(nextIndex);
  }
</script>

<div
  class="segmented-choice"
  class:is-compact={compact}
  role="radiogroup"
  aria-label={label}
  aria-busy={busy}
  style:--count={options.length}
  style:--index={Math.max(0, selected)}
>
  {#if selected >= 0}<span class="segmented-thumb" aria-hidden="true"></span>{/if}
  {#each options as option, index (option.value)}
    <button
      bind:this={buttons[index]}
      type="button"
      role="radio"
      aria-checked={value === option.value}
      class:active={value === option.value}
      tabindex={value === option.value ? 0 : -1}
      title={option.description || undefined}
      onclick={() => activate(index)}
      onkeydown={(event) => moveSelection(event, index)}
    >
      <span>{option.label}</span>
    </button>
  {/each}
</div>
