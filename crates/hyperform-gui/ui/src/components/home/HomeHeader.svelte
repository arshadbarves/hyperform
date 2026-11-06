<script lang="ts">
  import { projects } from "$lib/runtime";
  import { addWorkspace } from "$lib/stores/app-actions";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button, EmptyState } from "$lib/ui";
  import Icon, { FolderPlus, GitBranch } from "$lib/ui/icons";
  import { perch } from "$lib/ui/perch.svelte";
  import { wsBasename } from "$lib/workspaces";

  /**
   * Above the hero composer: the pet's spot (it sits here while you are
   * home), which project, which branch, and the one question.
   */
  type Props = { root: string | null };
  let { root }: Props = $props();

  const summary = $derived(projects.summary(root));
  const lively = $derived((settingsStore.settings?.ui.companion ?? "lively") === "lively");
</script>

{#if root}
  <header class="home-head">
    {#if lively}<div class="home-pet" use:perch={{ id: "hero", kind: "slot" }} aria-hidden="true"></div>{/if}
    <p class="home-eyebrow">
      <span class="home-project">{wsBasename(root)}</span>
      {#if summary?.branch}
        <span class="home-branch"><Icon icon={GitBranch} size={11} strokeWidth={2} />{summary.branch}</span>
      {/if}
    </p>
    <h1 class="home-title">What should we work on?</h1>
  </header>
{:else}
  <EmptyState
    icon={FolderPlus}
    title="Add a project to start"
    description="HyperForm works inside a folder on your computer: it reads, edits and runs the code there."
    class="home-empty"
  >
    <Button variant="accent" onclick={() => void addWorkspace()}>Add a project</Button>
  </EmptyState>
{/if}
