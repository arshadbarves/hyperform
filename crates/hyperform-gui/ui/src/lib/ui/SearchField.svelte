<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import Icon, { Search, X } from "./icons";

  /**
   * The one search field: a magnifier, the query and a clear button, in the
   * Settings nav and the prompt inspector's outline.
   */
  type Props = Omit<HTMLInputAttributes, "value" | "size" | "class"> & {
    value: string;
    /** Names the field for assistive tech. */
    label: string;
    size?: "s" | "m";
    class?: string;
    input?: HTMLInputElement;
    onclear?: () => void;
  };

  let {
    value = $bindable(""),
    label,
    size = "m",
    class: className = "",
    input = $bindable(),
    onclear,
    ...rest
  }: Props = $props();

  const iconSize = $derived(size === "s" ? 12 : 14);

  function clear() {
    value = "";
    onclear?.();
    input?.focus();
  }
</script>

<label class={`search-field size-${size} ${className}`}>
  <Icon icon={Search} size={iconSize} />
  <input
    bind:this={input}
    bind:value
    type="text"
    aria-label={label}
    autocomplete="off"
    spellcheck="false"
    {...rest}
  />
  {#if value}
    <button type="button" class="search-field-clear" aria-label="Clear search" onclick={clear}>
      <Icon icon={X} size={11} />
    </button>
  {/if}
</label>
