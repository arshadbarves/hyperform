<script lang="ts">
  import { Button, Dialog, DialogPanel } from "$lib/ui";
  import { GitBranch } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";

  /**
   * A new chat on its own branch: a second checkout of the project, so its
   * changes stay apart from your working tree until you merge them.
   */
  type Props = {
    projects: string[];
    project: string | null;
    onClose: () => void;
    onCreate: (name: string, from: string | null) => Promise<void>;
  };
  let { projects, project, onClose, onCreate }: Props = $props();

  let name = $state("");
  let from = $state<string | null>(null);
  let creating = $state(false);
  const source = $derived(from ?? project ?? projects[0] ?? null);
  const slug = $derived(
    name
      .trim()
      .toLowerCase()
      .replace(/\s+/g, "-")
      .replace(/[^a-z0-9-]/g, "")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, ""),
  );

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!slug || creating) return;
    creating = true;
    await onCreate(slug, source);
    creating = false;
    onClose();
  }
</script>

<Dialog.Root
  open={true}
  onOpenChange={(open) => {
    if (!open) onClose();
  }}
>
  <DialogPanel
    title="New chat in a worktree"
    description="A second copy of the project on its own branch. The chat's changes stay apart from your working tree until you merge them."
    icon={GitBranch}
    contentClass="worktree-dialog"
  >
    <form class="modal-body" onsubmit={submit}>
      {#if projects.length > 1}
        <label class="modal-field">
          <span class="modal-field-label">Project</span>
          <select class="text-field" value={source} onchange={(e) => (from = e.currentTarget.value)}>
            {#each projects as root (root)}<option value={root}>{wsBasename(root)}</option>{/each}
          </select>
        </label>
      {/if}
      <label class="modal-field">
        <span class="modal-field-label">What is it for?</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="text-field" bind:value={name} placeholder="fix auth redirect" spellcheck={false} autofocus />
      </label>
      <p class="worktree-preview" aria-live="polite">
        {#if slug}
          Branch <code>hyperform/{slug}</code> in <code>.hyperform/worktrees/{slug}</code>
        {:else}
          A short name becomes the branch and folder name.
        {/if}
      </p>
      <footer class="modal-footer">
        <Button variant="secondary" onclick={onClose}>Cancel</Button>
        <Button type="submit" variant="accent" disabled={!slug || !source || creating}>
          {creating ? "Creating…" : "Create and start"}
        </Button>
      </footer>
    </form>
  </DialogPanel>
</Dialog.Root>
