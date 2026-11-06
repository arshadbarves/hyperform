<script lang="ts">
  import { startersFor, type Starter } from "$lib/domain/starters";
  import { projects } from "$lib/runtime";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import Icon, { ArrowRight } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";
  import StarterIcon from "../home/StarterIcon.svelte";
  import StepLayout from "./StepLayout.svelte";

  /** The end of setup: a few good first prompts for the project, or straight in. */
  type Props = { onFinish: (prompt: string | null) => void; onBack: () => void };
  let { onFinish, onBack }: Props = $props();

  const root = $derived(onboarding.project);
  const summary = $derived(projects.summary(root));
  const starters: Starter[] = $derived(root ? startersFor({ changed: summary?.changed ?? 0, hasInstructions: null }).top : []);
</script>

<StepLayout
  title="You're all set"
  lead={root ? `Here are a few ways to begin in ${wsBasename(root)}.` : "Add a project from the sidebar whenever you are ready."}
>
  {#if starters.length}
    <div class="ready-starters">
      {#each starters as starter (starter.id)}
        <button type="button" class="ready-starter" onclick={() => onFinish(starter.prompt)}>
          <StarterIcon kind={starter.kind} />
          <span class="ready-starter-title">{starter.title}</span>
          <Icon icon={ArrowRight} size={13} class="ready-starter-go" />
        </button>
      {/each}
    </div>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn-ghost" onclick={onBack}>Back</button>
    <button type="button" class="btn-accent size-l" onclick={() => onFinish(null)}>Open HyperForm</button>
  {/snippet}
</StepLayout>
