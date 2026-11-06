<script lang="ts">
  import { openPath, revealPath } from "$lib/commands";
  import { absolutePath } from "$lib/domain/paths";
  import { revealLabel } from "$lib/platform";
  import { errorText, pushToast } from "$lib/runtime";
  import { Menu } from "$lib/ui";
  import Icon, { Check, Copy, ExternalLink, FileCode, MoreHorizontal } from "$lib/ui/icons";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";

  /** The reviewed file: where it lives, how much changed, what you can do with it. */
  type Props = { path: string; root: string | null; added: number; deleted: number; deletedFile: boolean; diff: string };
  let { path, root, added, deleted, deletedFile, diff }: Props = $props();

  const copied = copyFeedback();
  const cut = $derived(path.lastIndexOf("/"));
  const dir = $derived(cut > 0 ? path.slice(0, cut + 1) : "");
  const name = $derived(path.slice(cut + 1));
  const full = $derived(root ? absolutePath(root, path) : path);
  const canOpen = $derived(Boolean(root) && !deletedFile);

  async function run(action: () => Promise<void>, failed: string) {
    try {
      await action();
    } catch (e) {
      pushToast(`${failed} · ${errorText(e)}`, "error");
    }
  }

  async function copyText(text: string, what: string) {
    if (!(await copied.copy(text))) pushToast(`Could not copy the ${what}`, "error");
  }
</script>

<div class="diff-filebar">
  <Icon icon={FileCode} size={13} class="diff-filebar-icon" />
  <span class="diff-filebar-path" title={full}>
    {#if dir}<span class="diff-filebar-dir">{dir}</span>{/if}<span class="diff-filebar-name">{name}</span>
  </span>
  <span class="diff-filebar-stat">
    {#if added}<span class="diff-add-text">+{added}</span>{/if}
    {#if deleted}<span class="diff-del-text">−{deleted}</span>{/if}
  </span>
  <span class="diff-filebar-spacer"></span>
  {#if canOpen}
    <button type="button" class="btn-ghost diff-filebar-btn" onclick={() => void run(() => openPath(full), "Could not open the file")}>
      <Icon icon={ExternalLink} size={12} />
      Open
    </button>
  {/if}
  <button type="button" class="icon-btn" aria-label="Copy the diff" title="Copy the diff" onclick={() => void copyText(diff, "diff")}>
    <Icon icon={copied.copied ? Check : Copy} size={13} />
  </button>
  <Menu.Root>
    <Menu.Trigger class="icon-btn" aria-label="More file actions">
      <Icon icon={MoreHorizontal} size={14} />
    </Menu.Trigger>
    <Menu.Portal>
      <Menu.Content class="menu" side="bottom" align="end" sideOffset={6}>
        {#if canOpen}
          <Menu.Item class="menu-item" onSelect={() => void run(() => revealPath(full), "Could not show the file")}>
            {revealLabel()}
          </Menu.Item>
        {/if}
        <Menu.Item class="menu-item" onSelect={() => void copyText(full, "path")}>Copy path</Menu.Item>
        <Menu.Item class="menu-item" onSelect={() => void copyText(path, "path")}>Copy relative path</Menu.Item>
        <Menu.Item class="menu-item" onSelect={() => void copyText(diff, "diff")}>Copy diff</Menu.Item>
      </Menu.Content>
    </Menu.Portal>
  </Menu.Root>
</div>
