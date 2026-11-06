<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    class?: string;
    children: Snippet;
  };

  let { class: className = "", children }: Props = $props();
</script>

<div class={`settings-card ${className}`.trim()}>
  {@render children()}
</div>
