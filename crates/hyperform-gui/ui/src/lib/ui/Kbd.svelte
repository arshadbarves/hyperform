<script lang="ts">
  /** A keyboard shortcut, e.g. "⌘K" or "Esc". */
  type Props = { keys: string; class?: string };

  let { keys, class: className = "" }: Props = $props();
</script>

<kbd class={`kbd ${className}`}>{keys}</kbd>
