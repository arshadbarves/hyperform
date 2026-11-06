<script lang="ts">
  import { resolveSuggestion, saveStandingRule, type RuleScope } from "$lib/runtime";
  import { Button } from "$lib/ui";
  import Icon, { Bookmark } from "$lib/ui/icons";

  /** `decisions_memory_suggest`: nothing is written until the user picks a file. */
  type Props = { suggestionId: string; rule: string; projectRoot: string | null };
  let { suggestionId, rule, projectRoot }: Props = $props();

  let sending = $state(false);

  async function save(scope: RuleScope) {
    if (sending || !projectRoot) return;
    sending = true;
    if (!(await saveStandingRule(projectRoot, scope, rule))) {
      sending = false;
      return;
    }
    if (!(await resolveSuggestion(suggestionId, true))) sending = false;
  }

  async function dismiss() {
    if (sending) return;
    sending = true;
    if (!(await resolveSuggestion(suggestionId, false))) sending = false;
  }
</script>

<div class="interaction-card suggestion-card" role="region" aria-label="Standing rule suggestion">
  <div class="interaction-kicker">
    <Icon icon={Bookmark} size={13} />
    <span>Remember this for next time?</span>
  </div>
  <blockquote class="suggestion-rule">{rule}</blockquote>
  <div class="interaction-actions">
    <Button variant="accent" disabled={sending || !projectRoot} onclick={() => void save("project")}>
      Save to AGENTS.md
    </Button>
    <Button disabled={sending || !projectRoot} onclick={() => void save("local")}>Save just for me</Button>
    <Button variant="ghost" disabled={sending} onclick={() => void dismiss()}>Not now</Button>
  </div>
</div>
