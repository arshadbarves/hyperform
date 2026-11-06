<script lang="ts">
  import { resolveSuggestion, runCommand } from "$lib/runtime";
  import { Button } from "$lib/ui";
  import Icon, { ShieldAlert } from "$lib/ui/icons";

  /** `decisions_review_suggest`: the review runs only when the user clicks, once the turn is over. */
  type Props = { suggestionId: string; areas: string[]; paths: string[]; busy: boolean };
  let { suggestionId, areas, paths, busy }: Props = $props();

  const SHOWN_PATHS = 3;
  let sending = $state(false);
  const more = $derived(paths.length - SHOWN_PATHS);

  async function answer(accepted: boolean) {
    if (sending) return;
    sending = true;
    if (accepted && !(await runCommand("review"))) {
      sending = false;
      return;
    }
    if (!(await resolveSuggestion(suggestionId, accepted))) sending = false;
  }
</script>

<div class="interaction-card suggestion-card" role="region" aria-label="Review suggestion">
  <div class="interaction-kicker">
    <Icon icon={ShieldAlert} size={13} />
    <span>These changes touch {areas.join(", ")}. Run a review?</span>
  </div>
  {#if paths.length > 0}
    <p class="suggestion-text">
      {#each paths.slice(0, SHOWN_PATHS) as path, i (path)}{#if i > 0}, {/if}<code>{path}</code>{/each}
      {#if more > 0}and {more} more{/if}
    </p>
  {/if}
  <div class="interaction-actions">
    <Button variant="accent" disabled={sending || busy} onclick={() => void answer(true)}>Run review</Button>
    <Button disabled={sending} onclick={() => void answer(false)}>Not now</Button>
  </div>
</div>
