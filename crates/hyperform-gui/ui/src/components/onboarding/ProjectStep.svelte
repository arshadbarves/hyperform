<script lang="ts">
  import { setTrust } from "$lib/commands";
  import { errorText, projects, pushToast } from "$lib/runtime";
  import { onPathDrop } from "$lib/runtime/pathDrop";
  import { addWorkspace, addWorkspaceAt } from "$lib/stores/app-actions";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import Icon, { Check, FolderOpen, GitBranch, Shield } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";
  import StepLayout from "./StepLayout.svelte";

  /** The folder to work in, dropped or chosen, and whether to trust it. */
  type Props = { onNext: () => void; onBack: () => void };
  let { onNext, onBack }: Props = $props();

  let hovering = $state(false);
  let trust = $state<"ask" | "trusted" | "later">("ask");
  const root = $derived(onboarding.project);
  const summary = $derived(projects.summary(root));

  $effect(() => {
    let unlisten: (() => void) | null = null;
    let gone = false;
    onPathDrop({ hover: (over) => (hovering = over), drop: (paths) => void adopt(paths[0]) })
      .then((stop) => (gone ? stop() : (unlisten = stop)))
      .catch(() => {});
    return () => {
      gone = true;
      unlisten?.();
    };
  });

  async function adopt(path: string | undefined) {
    if (!path) return;
    try {
      const added = await addWorkspaceAt(path);
      if (added) chosen(added);
    } catch (e) {
      pushToast(`That did not work: ${errorText(e)}. Drop a folder, not a file.`, "warn");
    }
  }

  async function choose() {
    const added = await addWorkspace();
    if (added) chosen(added);
  }

  function chosen(path: string) {
    onboarding.project = path;
    trust = "ask";
    void projects.refresh(path);
  }

  async function answer(trusted: boolean) {
    if (trusted && root) {
      try {
        await setTrust(root, true);
      } catch (e) {
        pushToast(`Could not trust the project: ${errorText(e)}`, "warn");
        return;
      }
    }
    trust = trusted ? "trusted" : "later";
  }
</script>

<StepLayout title="Pick a project to work on" lead="HyperForm works inside one folder at a time. You can add more later.">
  {#if !root}
    <button type="button" class="drop-zone" class:is-hovering={hovering} onclick={() => void choose()}>
      <span class="drop-zone-icon"><Icon icon={FolderOpen} size={22} strokeWidth={1.6} /></span>
      <span class="drop-zone-title">{hovering ? "Drop to add this folder" : "Drop a project folder here"}</span>
      <span class="drop-zone-hint">or click to choose one</span>
    </button>
  {:else}
    <div class="project-card">
      <span class="project-card-icon"><Icon icon={FolderOpen} size={18} strokeWidth={1.6} /></span>
      <span class="project-card-text">
        <span class="project-card-name">{wsBasename(root)}</span>
        <span class="project-card-path">{root}</span>
      </span>
      {#if summary?.branch}
        <span class="project-card-branch"><Icon icon={GitBranch} size={11} /> {summary.branch}</span>
      {/if}
      <button type="button" class="btn-ghost" onclick={() => void choose()}>Change</button>
    </div>

    <div class="trust-card" class:is-answered={trust !== "ask"}>
      <span class="trust-icon"><Icon icon={trust === "trusted" ? Check : Shield} size={16} /></span>
      <div class="trust-text">
        {#if trust === "trusted"}
          <p class="trust-title">Trusted</p>
          <p class="trust-desc">This project's own settings, hooks and tools can run.</p>
        {:else if trust === "later"}
          <p class="trust-title">Not trusted for now</p>
          <p class="trust-desc">Its own hooks and tools stay off. HyperForm asks again when they matter.</p>
        {:else}
          <p class="trust-title">Do you trust this folder?</p>
          <p class="trust-desc">
            A trusted project can use its own settings, hooks and tool servers. Only trust code you know.
          </p>
          <div class="trust-actions">
            <button type="button" class="btn-ghost" onclick={() => void answer(false)}>Not now</button>
            <button type="button" class="btn-secondary" onclick={() => void answer(true)}>Trust it</button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn-ghost" onclick={onBack}>Back</button>
    <button type="button" class="btn-accent size-l" onclick={onNext}>{root ? "Continue" : "Skip for now"}</button>
  {/snippet}
</StepLayout>
