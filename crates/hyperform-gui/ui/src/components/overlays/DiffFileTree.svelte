<script lang="ts">
  import {
    buildDiffTree,
    expandAncestors,
    filterDiffTree,
    type DiffTreeFile,
    type DiffTreeNode,
  } from "$lib/domain/diffTree";
  import Icon, { ChevronDown, ChevronRight, Search, X } from "$lib/ui/icons";

  /** The changed files as a folder tree, with a filter; `[` and `]` step through them. */
  type Props = {
    files: DiffTreeFile[];
    selectedPath: string | null;
    searchQuery: string;
    onSelect: (path: string) => void;
    onSearchChange: (query: string) => void;
  };
  let { files, selectedPath, searchQuery, onSelect, onSearchChange }: Props = $props();

  let collapsed = $state<Record<string, boolean>>({});
  let seededFor: string | null = null;

  const tree = $derived(filterDiffTree(buildDiffTree(files), searchQuery));

  $effect(() => {
    if (!selectedPath || seededFor === selectedPath) return;
    seededFor = selectedPath;
    const next = { ...collapsed };
    for (const p of expandAncestors(selectedPath)) delete next[p];
    collapsed = next;
  });

  function isOpen(path: string): boolean {
    return Boolean(searchQuery.trim()) || !collapsed[path];
  }

  const LETTER: Record<string, string> = { added: "A", deleted: "D", renamed: "R" };
</script>

<nav class="diff-files" aria-label="Changed files">
  {#if files.length > 6}
    <label class="diff-files-filter">
      <Icon icon={Search} size={12} />
      <input
        type="text"
        value={searchQuery}
        oninput={(e) => onSearchChange(e.currentTarget.value)}
        placeholder="Filter files"
        spellcheck={false}
        aria-label="Filter files"
      />
      {#if searchQuery}
        <button type="button" class="diff-files-clear" aria-label="Clear the filter" onclick={() => onSearchChange("")}>
          <Icon icon={X} size={10} />
        </button>
      {/if}
    </label>
  {/if}

  <div class="diff-files-scroll">
    {#if tree.length === 0}
      <p class="diff-files-empty">No file matches “{searchQuery}”.</p>
    {:else}
      {#snippet nodes(list: DiffTreeNode[], depth: number)}
        {#each list as node (node.path + node.kind)}
          {#if node.kind === "dir"}
            {@const open = isOpen(node.path)}
            <button
              type="button"
              class="diff-tree-row is-dir"
              style={`--depth: ${depth}`}
              aria-expanded={open}
              onclick={() => (collapsed = { ...collapsed, [node.path]: !collapsed[node.path] })}
            >
              <Icon icon={open ? ChevronDown : ChevronRight} size={11} class="diff-tree-chevron" />
              <span class="diff-tree-name">{node.name}</span>
            </button>
            {#if open}{@render nodes(node.children, depth + 1)}{/if}
          {:else}
            <button
              type="button"
              class={`diff-tree-row is-file status-${node.status}`}
              class:is-selected={selectedPath === node.path}
              aria-current={selectedPath === node.path ? "true" : undefined}
              style={`--depth: ${depth}`}
              title={node.path}
              onclick={() => onSelect(node.path)}
            >
              <span class="diff-tree-letter" aria-label={node.status}>{LETTER[node.status] ?? "M"}</span>
              <span class="diff-tree-name">{node.name}</span>
              {#if node.added || node.deleted}
                <span class="diff-tree-stat">
                  {#if node.added}<span class="diff-add-text">+{node.added}</span>{/if}
                  {#if node.deleted}<span class="diff-del-text">−{node.deleted}</span>{/if}
                </span>
              {/if}
            </button>
          {/if}
        {/each}
      {/snippet}
      {@render nodes(tree, 0)}
    {/if}
  </div>
</nav>
