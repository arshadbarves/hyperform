<script lang="ts">
  import type { MentionItem } from "$lib/domain/composerPopover";
  import Icon, {
    Bot,
    Eye,
    FileCode,
    FileText,
    Folder,
    FolderGit2,
    LoaderCircle,
    Search,
    Sliders,
    Sparkles,
    Wrench,
    type IconSvgElement,
  } from "$lib/ui/icons";

  type Props = {
    items: MentionItem[];
    loading: boolean;
    selected: number;
    onPick: (item: MentionItem) => void;
    onHover: (index: number) => void;
  };
  let { items, loading, selected, onPick, onHover }: Props = $props();

  let listEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    void selected;
    listEl?.querySelector(".file-pop-item.sel")?.scrollIntoView({ block: "nearest" });
  });

  function splitPath(fullPath: string) {
    const clean = fullPath.replace(/^\/+/, "");
    const idx = clean.lastIndexOf("/");
    const name = idx === -1 ? clean : clean.slice(idx + 1);
    const dot = name.lastIndexOf(".");
    return { dir: idx === -1 ? "" : clean.slice(0, idx + 1), name, ext: dot > 0 ? name.slice(dot + 1).toLowerCase() : "" };
  }

  function fileLook(ext: string): { icon: IconSvgElement; colorClass: string } {
    if (["ts", "tsx", "js", "jsx"].includes(ext)) return { icon: FileCode, colorClass: ext.startsWith("t") ? "ext-ts" : "ext-js" };
    if (ext === "svelte") return { icon: Sparkles, colorClass: "ext-svelte" };
    if (ext === "rs") return { icon: Wrench, colorClass: "ext-rs" };
    if (["json", "toml", "yaml", "yml"].includes(ext)) return { icon: Sliders, colorClass: "ext-cfg" };
    if (["md", "txt"].includes(ext)) return { icon: FileText, colorClass: "ext-doc" };
    if (["png", "jpg", "jpeg", "svg", "gif"].includes(ext)) return { icon: Eye, colorClass: "ext-img" };
    return { icon: FileCode, colorClass: "ext-default" };
  }
</script>

<div class="composer-pop file-pop glass-strong" role="listbox" aria-label="Files and agents">
  <div class="file-pop-header">
    <div class="file-pop-header-title">
      <Icon icon={FolderGit2} size={13} class="file-header-icon" />
      <span>Mention a file or agent</span>
    </div>
    {#if !loading}<span class="file-count-pill">{items.length}</span>{/if}
  </div>

  <div class="file-pop-list" bind:this={listEl}>
    {#each items as item, i (item.kind === "agent" ? `agent:${item.agent.name}` : item.path)}
      <button
        type="button"
        role="option"
        aria-selected={i === selected}
        class={`file-pop-item ${item.kind === "agent" ? "ext-agent" : fileLook(splitPath(item.path).ext).colorClass}${i === selected ? " sel" : ""}`}
        onmouseenter={() => onHover(i)}
        onclick={() => onPick(item)}
      >
        {#if item.kind === "agent"}
          <div class="file-icon-badge"><Icon icon={Bot} size={13} strokeWidth={1.8} /></div>
          <div class="file-details">
            <div class="file-name-row">
              <span class="file-name-bold">@agent-{item.agent.name}</span>
              <span class="file-ext-chip">{item.agent.source}</span>
            </div>
            <div class="file-breadcrumb" title={item.agent.description}><span>{item.agent.description}</span></div>
          </div>
        {:else}
          {@const parts = splitPath(item.path)}
          <div class="file-icon-badge"><Icon icon={fileLook(parts.ext).icon} size={13} strokeWidth={1.8} /></div>
          <div class="file-details">
            <div class="file-name-row">
              <span class="file-name-bold">{parts.name}</span>
              {#if parts.ext}<span class="file-ext-chip">{parts.ext}</span>{/if}
            </div>
            {#if parts.dir}
              <div class="file-breadcrumb" title={parts.dir}>
                <Icon icon={Folder} size={10} class="crumb-folder-icon" />
                <span>{parts.dir}</span>
              </div>
            {/if}
          </div>
        {/if}
        {#if i === selected}<div class="file-insert-pill"><kbd>↵</kbd></div>{/if}
      </button>
    {/each}
    {#if loading}
      <div class="pop-loading"><Icon icon={LoaderCircle} size={14} class="spin" /><span>Searching workspace…</span></div>
    {:else if items.length === 0}
      <div class="pop-empty"><Icon icon={Search} size={14} /><span>No files or agents found</span></div>
    {/if}
  </div>

  <div class="file-pop-footer">
    <div class="file-footer-left">
      <span class="footer-hint"><kbd>↑↓</kbd> navigate</span>
      <span class="footer-hint"><kbd>↵</kbd> insert</span>
    </div>
    <span class="footer-hint"><kbd>Esc</kbd> close</span>
  </div>
</div>
