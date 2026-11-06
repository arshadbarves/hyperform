<script lang="ts">
  import type { RepoSummary } from "$lib/commands";
  import type { ProjectGroup } from "$lib/domain/sidebarModel";
  import { revealLabel } from "$lib/platform";
  import { pushToast } from "$lib/runtime";
  import { revealPath } from "$lib/commands";
  import { ContextMenu } from "$lib/ui";
  import Icon, { ChevronDown, ChevronRight, GitBranch, Plus } from "$lib/ui/icons";
  import ChatRow from "./ChatRow.svelte";

  /** A project: name, branch and uncommitted count, then its chats. Right-click for more. */
  type Props = {
    project: ProjectGroup;
    open: boolean;
    summary: RepoSummary | null;
    onToggle: () => void;
    onShowMore: () => void;
    onOpenChat: (sessionId: string, root: string) => void;
    onDeleteChat: (sessionId: string, title: string) => void;
    onNewChat: () => void;
    onNewWorktree: () => void;
    onRemove: () => void;
  };
  let { project, open, summary, onToggle, onShowMore, onOpenChat, onDeleteChat, onNewChat, onNewWorktree, onRemove }: Props =
    $props();

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    e.preventDefault();
    onToggle();
  }

  async function copyPath() {
    await navigator.clipboard.writeText(project.root);
    pushToast("Path copied", "ok");
  }

  function reveal() {
    revealPath(project.root).catch(() => pushToast(`Could not open ${project.name}`, "warn"));
  }
</script>

<div class={`project${project.active ? " is-active" : ""}`}>
  <ContextMenu.Root>
    <ContextMenu.Trigger>
      {#snippet child({ props })}
        <div
          {...props}
          class="project-head"
          role="button"
          tabindex={0}
          title={project.root}
          aria-expanded={open}
          onclick={onToggle}
          onkeydown={onKey}
        >
          <Icon icon={open ? ChevronDown : ChevronRight} size={11} strokeWidth={2} class="project-chevron" />
          <span class="project-name">{project.name}</span>
          {#if summary?.branch}
            <span class="project-branch" title={`On branch ${summary.branch}`}>
              <Icon icon={GitBranch} size={10} strokeWidth={2} />
              <span>{summary.branch}</span>
            </span>
          {/if}
          {#if summary?.changed}
            <span class="project-changes" title={`${summary.changed} uncommitted change${summary.changed === 1 ? "" : "s"}`}>
              {summary.changed}
            </span>
          {/if}
          {#if !open && project.mark}
            <span class={`chat-mark tone-${project.mark.tone}`} role="img" aria-label={project.mark.label}></span>
          {/if}
          <button
            type="button"
            class="sidebar-icon-btn"
            aria-label={`New chat in ${project.name}`}
            title="New chat here"
            onclick={(e) => {
              e.stopPropagation();
              onNewChat();
            }}
          >
            <Icon icon={Plus} size={12} strokeWidth={2} />
          </button>
        </div>
      {/snippet}
    </ContextMenu.Trigger>
    <ContextMenu.Portal>
      <ContextMenu.Content class="menu">
        <ContextMenu.Item class="menu-item" onSelect={onNewChat}>New chat here</ContextMenu.Item>
        <ContextMenu.Item class="menu-item" onSelect={onNewWorktree}>New chat in a worktree…</ContextMenu.Item>
        <ContextMenu.Separator class="menu-sep" />
        <ContextMenu.Item class="menu-item" onSelect={reveal}>{revealLabel()}</ContextMenu.Item>
        <ContextMenu.Item class="menu-item" onSelect={() => void copyPath()}>Copy path</ContextMenu.Item>
        <ContextMenu.Separator class="menu-sep" />
        <ContextMenu.Item class="menu-item is-danger" onSelect={onRemove}>Remove project…</ContextMenu.Item>
      </ContextMenu.Content>
    </ContextMenu.Portal>
  </ContextMenu.Root>

  {#if open}
    <div class="project-chats">
      {#each project.rows as row (row.item.sessionId)}
        <ChatRow
          {row}
          onOpen={() => onOpenChat(row.item.sessionId, row.item.projectRoot)}
          onDelete={() => onDeleteChat(row.item.sessionId, row.item.title)}
        />
      {:else}
        <button type="button" class="project-empty" onclick={onNewChat}>Start the first chat</button>
      {/each}
      {#if project.hidden}
        <button type="button" class="sidebar-more" onclick={onShowMore}>Show {project.hidden} more</button>
      {/if}
    </div>
  {/if}
</div>
