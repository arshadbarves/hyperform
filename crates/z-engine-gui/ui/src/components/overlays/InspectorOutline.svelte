<script lang="ts">
  import type { OutlineGroup } from "$lib/domain/inspectOutline";
  import { categoryMeta } from "$lib/promptInspectView";
  import { SearchField } from "$lib/ui";
  import { fmtTokens } from "$lib/util";

  /** The request's parts, grouped; ↑ and ↓ move through them, also from the search field. */
  type Props = {
    groups: OutlineGroup[];
    selected: number;
    query: string;
    total: number;
    onQuery: (query: string) => void;
    onSelect: (index: number) => void;
    onStep: (dir: -1 | 1) => void;
  };
  let { groups, selected, query, total, onQuery, onSelect, onStep }: Props = $props();

  let list: HTMLElement | undefined = $state();
  const shown = $derived(groups.reduce((n, g) => n + g.items.length, 0));

  $effect(() => {
    void selected;
    list?.querySelector<HTMLElement>("[aria-current='true']")?.scrollIntoView({ block: "nearest" });
  });

  function onKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    onStep(e.key === "ArrowDown" ? 1 : -1);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<nav class="inspector-outline" aria-label="Parts of the request" onkeydown={onKey}>
  <SearchField
    value={query}
    label="Search the request"
    placeholder="Search the request"
    size="s"
    oninput={(e) => onQuery(e.currentTarget.value)}
    onclear={() => onQuery("")}
  />
  {#if shown < total}<p class="inspector-outline-count">{shown} of {total} parts</p>{/if}

  <div class="inspector-outline-list" bind:this={list}>
    {#each groups as group (group.category)}
      <section class="inspector-group">
        <h3 class="inspector-group-title">
          <span class="inspector-swatch" style:background={categoryMeta(group.category).color}></span>
          {group.label}
          <span class="inspector-group-tokens">{fmtTokens(group.tokens)}</span>
        </h3>
        {#each group.items as item (item.index)}
          <button
            type="button"
            class="inspector-item"
            aria-current={selected === item.index ? "true" : undefined}
            onclick={() => onSelect(item.index)}
          >
            <span class="inspector-item-text">
              <span class="inspector-item-label" title={item.label}>{item.label}</span>
              {#if item.detail}<span class="inspector-item-detail">{item.detail}</span>{/if}
            </span>
            <span class="inspector-item-tokens">{fmtTokens(item.tokens)}</span>
          </button>
        {/each}
      </section>
    {:else}
      <p class="inspector-outline-empty">{query ? `Nothing in the request matches “${query}”.` : "Nothing of this kind in the request."}</p>
    {/each}
  </div>
</nav>
