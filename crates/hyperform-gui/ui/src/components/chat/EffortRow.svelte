<script lang="ts">
  import { EFFORTS } from "$lib/domain/modes";
  import type { Effort } from "$lib/protocol/Effort";
  import { setEffort } from "$lib/runtime";

  /** Reasoning effort as one row of choices, inside the model picker. */
  type Props = { effort: Effort | null };
  let { effort }: Props = $props();

  function pick(next: Effort | null) {
    if (next !== effort) void setEffort(next);
  }
</script>

<div class="effort-row" role="radiogroup" aria-label="Reasoning effort">
  <span class="effort-label">Effort</span>
  <button type="button" role="radio" aria-checked={effort === null} class="effort-choice" class:is-on={effort === null} title="Let the model decide" onclick={() => pick(null)}>
    auto
  </button>
  {#each EFFORTS as option (option.id)}
    <button
      type="button"
      role="radio"
      aria-checked={effort === option.id}
      class="effort-choice"
      class:is-on={effort === option.id}
      title={option.description}
      onclick={() => pick(option.id)}
    >
      {option.id}
    </button>
  {/each}
</div>
