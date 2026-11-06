<script lang="ts">
  import { PET_LOOKS, type PetLook } from "$lib/domain/pet/looks";

  /** The six looks as pearl swatches; one radio group, arrow keys move through it. */
  type Props = { value: PetLook; onSelect: (look: PetLook) => void; label?: string };
  let { value, onSelect, label = "Look" }: Props = $props();

  function onKey(e: KeyboardEvent) {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const index = PET_LOOKS.findIndex((l) => l.id === value);
    const next = PET_LOOKS[(index + step + PET_LOOKS.length) % PET_LOOKS.length];
    if (next) onSelect(next.id);
    const group = e.currentTarget as HTMLElement;
    queueMicrotask(() => group.querySelector<HTMLElement>("[aria-checked='true']")?.focus());
  }
</script>

<div class="pet-looks" role="radiogroup" aria-label={label} tabindex="-1" onkeydown={onKey}>
  {#each PET_LOOKS as look (look.id)}
    <button
      type="button"
      role="radio"
      class={`pet-look look-${look.id}`}
      aria-checked={value === look.id}
      aria-label={look.label}
      title={look.label}
      tabindex={value === look.id ? 0 : -1}
      onclick={() => onSelect(look.id)}
    >
      <span class="pet-look-orb" aria-hidden="true"></span>
    </button>
  {/each}
</div>
