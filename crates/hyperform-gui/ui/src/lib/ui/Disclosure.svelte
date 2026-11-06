<script lang="ts" module>
  import { Collapsible } from "bits-ui";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { ChevronRight } from "./icons";

  /**
   * One line of meaning at rest, the detail on demand. The body mounts only
   * while open, so long outputs cost nothing until asked for. Controls that
   * must stay clickable while closed go in `actions`, beside the trigger.
   */
  type Props = {
    open?: boolean;
    disabled?: boolean;
    chevron?: boolean;
    class?: string;
    summaryClass?: string;
    bodyClass?: string;
    label?: string;
    summary: Snippet<[boolean]>;
    actions?: Snippet;
    children: Snippet;
    onOpenChange?: (open: boolean) => void;
  };

  let {
    open = $bindable(false),
    disabled = false,
    chevron = true,
    class: className = "",
    summaryClass = "",
    bodyClass = "",
    label,
    summary,
    actions,
    children,
    onOpenChange,
  }: Props = $props();
</script>

<Collapsible.Root bind:open {disabled} {onOpenChange} class="disclosure {className}">
  <div class="disclosure-head">
    <Collapsible.Trigger class="disclosure-summary {summaryClass}" aria-label={label}>
      {#if chevron}
        <span class="disclosure-chevron" aria-hidden="true"><Icon icon={ChevronRight} size={11} strokeWidth={2} /></span>
      {/if}
      {@render summary(open)}
    </Collapsible.Trigger>
    {#if actions}<div class="disclosure-actions">{@render actions()}</div>{/if}
  </div>
  <Collapsible.Content class="disclosure-body {bodyClass}" hiddenUntilFound={false}>
    {#if open}{@render children()}{/if}
  </Collapsible.Content>
</Collapsible.Root>
