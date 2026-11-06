<script lang="ts">
  import { groupPalette, rankPalette } from "$lib/domain/palette";
  import type { SessionListItem } from "$lib/domain/sessionList";
  import { paletteSettings } from "$lib/paletteActions";
  import type { PaletteItem } from "$lib/paletteTypes";
  import { Dialog, DialogPanel, Kbd } from "$lib/ui";
  import { FolderGit2, Icon, MessageSquare, Search, X } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";

  /**
   * ⌘K: every action, chat, project and setting behind one search field.
   * Settings only join the list once you type, so the first view stays short.
   */
  type Props = {
    isClosing?: boolean;
    onClose: () => void;
    sessions: SessionListItem[];
    workspaces: string[];
    activeWorkspace: string | null;
    actions: PaletteItem[];
    /** `/resume`: list chats only. */
    sessionsOnly?: boolean;
    onOpenSession: (sessionId: string, projectRoot: string) => void;
    onActivateWorkspace: (root: string) => void;
  };

  let { isClosing = false, onClose, sessions, workspaces, activeWorkspace, actions, sessionsOnly = false, onOpenSession, onActivateWorkspace }: Props =
    $props();

  let query = $state("");
  let sel = $state(0);
  let listEl: HTMLDivElement | undefined = $state();
  const settingItems = paletteSettings();

  const items = $derived.by(() => {
    const chats: PaletteItem[] = sessions.slice(0, sessionsOnly ? 40 : 6).map((s) => ({
      label: s.title,
      hint: `${s.projectRoot ? wsBasename(s.projectRoot) : "Chat"}${s.legacy ? " · v1" : ""}`,
      keywords: `chat resume ${s.sessionId} ${s.projectRoot}`,
      group: "Recent chats",
      icon: MessageSquare,
      run: () => onOpenSession(s.sessionId, s.projectRoot),
    }));
    if (sessionsOnly) return rankPalette(chats, query);
    const projects: PaletteItem[] = workspaces.map((root) => ({
      label: wsBasename(root),
      hint: root === activeWorkspace ? "Current project" : "Switch to this project",
      keywords: `project workspace folder ${root}`,
      group: "Projects",
      icon: FolderGit2,
      run: () => onActivateWorkspace(root),
    }));
    const [lead, others] = groupsInOrder(actions);
    const all = [...lead, ...chats, ...others, ...projects, ...(query.trim() ? settingItems : [])];
    return rankPalette(all, query);
  });

  /** Actions and places first, then the recent chats, then the rest. */
  function groupsInOrder(list: PaletteItem[]): [PaletteItem[], PaletteItem[]] {
    const lead = list.filter((i) => i.group === "Actions" || i.group === "Go to");
    return [lead, list.filter((i) => !lead.includes(i))];
  }

  const selIndex = $derived(Math.min(sel, Math.max(0, items.length - 1)));
  const groups = $derived(groupPalette(items));

  $effect(() => {
    void selIndex;
    listEl?.querySelector(".palette-row.is-selected")?.scrollIntoView({ block: "nearest" });
  });

  function run(i: number) {
    const item = items[i];
    if (!item) return;
    onClose();
    item.run();
  }

  function onInputKey(e: KeyboardEvent) {
    const n = Math.max(1, items.length);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      sel = (selIndex + 1) % n;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      sel = (selIndex - 1 + n) % n;
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(selIndex);
    }
  }
</script>

<Dialog.Root
  open={true}
  onOpenChange={(open) => {
    if (!open) onClose();
  }}
>
  <DialogPanel label="Command palette" contentClass="palette" closing={isClosing}>
    <label class="palette-field">
      <Icon icon={Search} size={16} />
      <input
        bind:value={query}
        oninput={() => (sel = 0)}
        onkeydown={onInputKey}
        placeholder={sessionsOnly ? "Search chats to resume" : "Search actions, chats, projects and settings"}
        spellcheck={false}
        aria-label="Search"
        aria-controls="palette-list"
      />
      {#if query}
        <button type="button" class="icon-btn-mini" aria-label="Clear the search" onclick={() => ((query = ""), (sel = 0))}>
          <Icon icon={X} size={13} />
        </button>
      {/if}
    </label>

    <div class="palette-list" id="palette-list" role="listbox" bind:this={listEl}>
      {#if items.length === 0}
        <p class="palette-empty">Nothing matches “{query.trim()}”.</p>
      {:else}
        {#each groups as g (g.name ?? "_")}
          <section class="palette-group" aria-label={g.name}>
            {#if g.name}<h3 class="palette-group-title">{g.name}</h3>{/if}
            {#each g.items as { item, index } (`${item.group}-${item.label}-${index}`)}
              <button
                type="button"
                role="option"
                aria-selected={index === selIndex}
                class="palette-row"
                class:is-selected={index === selIndex}
                onmouseenter={() => (sel = index)}
                onclick={() => run(index)}
              >
                <Icon icon={item.icon ?? Search} size={15} class="palette-row-icon" />
                <span class="palette-row-label">{item.label}</span>
                {#if item.hint}<span class="palette-row-hint">{item.hint}</span>{/if}
                {#if item.shortcut}<Kbd keys={item.shortcut} />{/if}
              </button>
            {/each}
          </section>
        {/each}
      {/if}
    </div>

    <footer class="palette-foot">
      <span><Kbd keys="↑" /><Kbd keys="↓" /> move</span>
      <span><Kbd keys="↵" /> open</span>
      <span><Kbd keys="Esc" /> close</span>
    </footer>
  </DialogPanel>
</Dialog.Root>
