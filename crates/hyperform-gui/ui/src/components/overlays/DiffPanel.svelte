<script lang="ts">
  import { untrack } from "svelte";
  import { diffForFile, listChangedFiles, sessionChangedFiles, sessionDiffForFile, type ChangedFile } from "$lib/commands";
  import { summarizeFiles } from "$lib/domain/diffRows";
  import { buildDiffTree, filterDiffTree, flattenDiffTree } from "$lib/domain/diffTree";
  import { relPath } from "$lib/domain/tools/toolInput";
  import { errorText, sessions } from "$lib/runtime";
  import { ui, type DiffScope } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { EmptyState } from "$lib/ui";
  import { GitCompare } from "$lib/ui/icons";
  import { workspaceStore } from "$lib/workspaces";
  import DiffFileTree from "./DiffFileTree.svelte";
  import DiffHeader from "./DiffHeader.svelte";
  import DiffView from "./DiffView.svelte";

  /**
   * The side panel's Changes tab: this chat's edits or every uncommitted
   * change, a file list and the reviewed file. `[` / `]` (or `k` / `j`)
   * step through files.
   */

  const workspaces = bindStore(workspaceStore);
  const sessionId = $derived(sessions.activeId);
  const root = $derived(sessions.active?.info?.projectRoot ?? workspaces.current.active ?? null);
  const status = $derived(sessions.active?.status ?? "idle");
  let scope = $state<DiffScope>(ui.diffScope ?? (sessions.activeId ? "session" : "git"));
  let files = $state<ChangedFile[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let diffs = $state<Record<string, string>>({});
  let loadingDiff = $state(false);
  let refreshing = $state(false);
  let query = $state("");
  let filesOpen = $state(true);
  let gen = 0;

  async function loadFiles(): Promise<ChangedFile[]> {
    if (scope === "git") return listChangedFiles(root);
    if (!sessionId) return [];
    const changed = await sessionChangedFiles(sessionId);
    return changed.map((f) => ({ path: f.path, status: f.kind, added: 0, deleted: 0 }));
  }

  async function refresh(focus: string | null = null) {
    const mine = ++gen;
    refreshing = true;
    try {
      const next = await loadFiles();
      if (mine !== gen) return;
      files = next;
      error = null;
      diffs = {};
      const want = focus ? relPath(focus, root) : selected;
      const keep = want && next.some((f) => f.path === want) ? want : (next[0]?.path ?? null);
      selected = null;
      if (keep) void select(keep);
    } catch (e) {
      if (mine !== gen) return;
      error = errorText(e);
      files = [];
    } finally {
      if (mine === gen) refreshing = false;
    }
  }

  async function select(path: string) {
    selected = path;
    const key = `${scope}:${path}`;
    if (diffs[key] !== undefined) return;
    loadingDiff = true;
    try {
      const text = scope === "git" ? await diffForFile(path, root) : sessionId ? await sessionDiffForFile(sessionId, path) : "";
      diffs = { ...diffs, [key]: text };
    } catch (e) {
      diffs = { ...diffs, [key]: `No diff available: ${errorText(e)}` };
    } finally {
      loadingDiff = false;
    }
  }

  function setScope(next: DiffScope) {
    if (scope === next) return;
    scope = next;
    query = "";
    files = null;
    void refresh();
  }

  // Opening at a file (a receipt chip, a Changes card) or switching chats reloads.
  let shownFor: string | null | undefined;
  $effect(() => {
    const focus = ui.diffFocus;
    const wanted = ui.diffScope;
    const id = sessionId;
    untrack(() => {
      if (focus === null && wanted === null && id === shownFor) return;
      shownFor = id;
      if (wanted) scope = wanted;
      if (focus !== null) ui.diffFocus = null;
      if (wanted !== null) ui.diffScope = null;
      void refresh(focus);
    });
  });

  // A finished turn may have changed files: reload what is shown.
  let lastStatus: string | null = null;
  $effect(() => {
    const now = status;
    if (lastStatus && lastStatus !== "idle" && now === "idle") untrack(() => void refresh());
    lastStatus = now;
  });

  const order = $derived(files ? flattenDiffTree(filterDiffTree(buildDiffTree(files), query)) : []);

  function step(dir: -1 | 1) {
    if (order.length === 0) return;
    const at = order.indexOf(selected ?? "");
    const next = order[at === -1 ? 0 : Math.max(0, Math.min(order.length - 1, at + dir))];
    if (next) void select(next);
  }

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
      if (ui.settingsOpen || ui.paletteOpen) return;
      if ((e.target as HTMLElement | null)?.closest("textarea, input, [contenteditable='true'], [role='dialog'], [role='menu']")) return;
      if (e.key === "[" || e.key === "k") step(-1);
      else if (e.key === "]" || e.key === "j") step(1);
      else return;
      e.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const summary = $derived(summarizeFiles(files ?? []));
  const current = $derived(files?.find((f) => f.path === selected) ?? null);
  const text = $derived(selected ? (diffs[`${scope}:${selected}`] ?? null) : null);
  const emptyTitle = $derived(scope === "session" ? "This chat changed nothing yet" : "Nothing uncommitted");
  const emptyText = $derived(
    scope === "session"
      ? sessionId
        ? "Files the agent edits in this chat show up here."
        : "Open a chat to review what it changed."
      : "The working tree matches the last commit.",
  );
</script>

<div class="diff-panel">
  <DiffHeader
    {scope}
    {summary}
    {refreshing}
    {filesOpen}
    onScope={setScope}
    onRefresh={() => void refresh()}
    onToggleFiles={() => (filesOpen = !filesOpen)}
  />

  {#if error}
    <EmptyState icon={GitCompare} title="Could not list the changes" description={error} />
  {:else if files === null}
    <p class="diff-loading">Looking for changes…</p>
  {:else if files.length === 0}
    <EmptyState icon={GitCompare} title={emptyTitle} description={emptyText} />
  {:else}
    <div class="diff-body" class:has-files={filesOpen && files.length > 1}>
      {#if filesOpen && files.length > 1}
        <DiffFileTree {files} selectedPath={selected} searchQuery={query} onSelect={(p) => void select(p)} onSearchChange={(q) => (query = q)} />
      {/if}
      <div class="diff-viewer">
        {#if selected && text !== null}
          {#key `${scope}:${selected}`}
            <DiffView {text} path={selected} {root} deletedFile={current?.status === "deleted"} layout={ui.diffLayout} />
          {/key}
        {:else if loadingDiff}
          <p class="diff-loading">Loading {selected}…</p>
        {/if}
      </div>
    </div>
  {/if}
</div>
