<script lang="ts">
  import type { SlashCommandInfo } from "$lib/commands";
  import { groupCommands, kindTag, sourceTag } from "$lib/domain/slashCommands";
  import Icon, { Plug, Search, Sliders, Sparkles, Zap, type IconSvgElement } from "$lib/ui/icons";

  type Props = {
    /** Already in menu order (see `menuOrder`), so indexes match the groups. */
    items: SlashCommandInfo[];
    selected: number;
    onPick: (command: SlashCommandInfo) => void;
    onHover: (index: number) => void;
  };
  let { items, selected, onPick, onHover }: Props = $props();

  let listEl: HTMLDivElement | undefined = $state();
  const groups = $derived(groupCommands(items));

  $effect(() => {
    void selected;
    listEl?.querySelector(".cmd-pop-item.sel")?.scrollIntoView({ block: "nearest" });
  });

  function iconFor(command: SlashCommandInfo): { icon: IconSvgElement; tone: string } {
    if (command.source === "mcp") return { icon: Plug, tone: "cmd-tool" };
    if (command.kind === "ui") return { icon: Sliders, tone: "cmd-status" };
    if (command.kind === "prompt") return { icon: Sparkles, tone: "cmd-custom" };
    return { icon: Zap, tone: "cmd-compact" };
  }
</script>

<div class="composer-pop command-pop glass-strong" role="listbox" aria-label="Slash commands">
  <div class="cmd-pop-header">
    <div class="cmd-pop-header-title">
      <Icon icon={Sparkles} size={13} class="cmd-glow-icon" />
      <span>Slash Commands</span>
    </div>
    <span class="cmd-count-pill">{items.length}</span>
  </div>

  <div class="cmd-pop-list" bind:this={listEl}>
    {#if items.length === 0}
      <div class="pop-empty">
        <Icon icon={Search} size={14} />
        <span>No matching commands</span>
      </div>
    {/if}
    {#each groups as group (group.kind)}
      <div class="cmd-group-label" role="presentation">{group.label}</div>
      {#each group.commands as command (command.name)}
        {@const i = items.indexOf(command)}
        {@const look = iconFor(command)}
        <button
          type="button"
          role="option"
          aria-selected={i === selected}
          class={`cmd-pop-item ${look.tone}${i === selected ? " sel" : ""}`}
          onmouseenter={() => onHover(i)}
          onclick={() => onPick(command)}
        >
          <div class="cmd-icon-box"><Icon icon={look.icon} size={13} strokeWidth={1.8} /></div>
          <div class="cmd-info-col">
            <div class="cmd-title-row">
              <span class="cmd-name">/{command.name}</span>
              {#if command.argumentHint}<span class="cmd-arg-hint">{command.argumentHint}</span>{/if}
              <span class="cmd-cat-tag" title="What running it does">{kindTag(command)}</span>
              {#if command.kind !== "ui"}
                <span class="cmd-cat-tag" title="Where it is defined">{sourceTag(command)}</span>
              {/if}
            </div>
            <span class="cmd-desc">{command.description}</span>
          </div>
          {#if i === selected}
            <div class="cmd-enter-pill"><kbd>↵</kbd></div>
          {/if}
        </button>
      {/each}
    {/each}
  </div>

  <div class="cmd-pop-footer">
    <div class="cmd-footer-left">
      <span class="footer-hint"><kbd>↑↓</kbd> navigate</span>
      <span class="footer-hint"><kbd>↵</kbd> run</span>
      <span class="footer-hint"><kbd>Tab</kbd> complete</span>
    </div>
    <span class="footer-hint"><kbd>Esc</kbd> close</span>
  </div>
</div>

<style>
  .cmd-group-label {
    padding: 8px 12px 3px;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--label-3);
  }
</style>
