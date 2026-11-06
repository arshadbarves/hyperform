<script lang="ts">
  import { resolveSuggestion, setPlanning } from "$lib/runtime";
  import { Button } from "$lib/ui";
  import Icon, { ListChecks } from "$lib/ui/icons";

  /** `decisions_plan_suggest`: Plan mode starts only when the user clicks. */
  type Props = { suggestionId: string };
  let { suggestionId }: Props = $props();

  let sending = $state(false);

  async function answer(accepted: boolean) {
    if (sending) return;
    sending = true;
    if (accepted && !(await setPlanning(true))) {
      sending = false;
      return;
    }
    if (!(await resolveSuggestion(suggestionId, accepted))) sending = false;
  }
</script>

<div class="interaction-card suggestion-card" role="region" aria-label="Plan suggestion">
  <div class="interaction-kicker">
    <Icon icon={ListChecks} size={13} />
    <span>This looks like a large change. Plan first?</span>
  </div>
  <p class="suggestion-text">
    Plan mode explores read-only and proposes a plan for you to review before anything is edited.
  </p>
  <div class="interaction-actions">
    <Button variant="accent" disabled={sending} onclick={() => void answer(true)}>Plan first</Button>
    <Button disabled={sending} onclick={() => void answer(false)}>Not now</Button>
  </div>
</div>
