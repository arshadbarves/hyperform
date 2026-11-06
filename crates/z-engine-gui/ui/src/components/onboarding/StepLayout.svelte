<script lang="ts">
  import type { Snippet } from "svelte";

  /** One setup step: a short heading, the choice, and the way forward. */
  type Props = { title: string; lead?: string; children?: Snippet; footer: Snippet };
  let { title, lead, children, footer }: Props = $props();
</script>

<div class="step">
  <header class="step-head">
    <h1 class="step-title">{title}</h1>
    {#if lead}<p class="step-lead">{lead}</p>{/if}
  </header>
  {#if children}<div class="step-body">{@render children()}</div>{/if}
  <footer class="step-foot">{@render footer()}</footer>
</div>
