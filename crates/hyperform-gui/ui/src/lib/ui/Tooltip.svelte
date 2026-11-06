<script lang="ts">
  import { Tooltip } from "bits-ui";
  import type { Snippet } from "svelte";
  import Kbd from "./Kbd.svelte";

  type Props = {
    text: string;
    /** Keyboard shortcut shown after the text, e.g. "⌘K". */
    shortcut?: string;
    side?: "top" | "bottom" | "left" | "right";
    children: Snippet;
  };

  let { text, shortcut, side = "bottom", children }: Props = $props();
</script>

<Tooltip.Provider>
  <Tooltip.Root delayDuration={280}>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <span {...props} class="tip-anchor">
          {@render children()}
        </span>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Portal>
      <Tooltip.Content class="tip" {side} sideOffset={6}>
        <span>{text}</span>
        {#if shortcut}<Kbd keys={shortcut} />{/if}
      </Tooltip.Content>
    </Tooltip.Portal>
  </Tooltip.Root>
</Tooltip.Provider>
