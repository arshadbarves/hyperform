<script lang="ts">
  /** A count or a short word; a zero count draws nothing. */
  type Props = {
    count?: number;
    text?: string;
    tone?: "neutral" | "attention" | "danger" | "ok" | "info";
    label?: string;
    class?: string;
  };

  let { count, text, tone = "neutral", label, class: className = "" }: Props = $props();

  const shown = $derived(text ?? (count === undefined || count <= 0 ? "" : count > 99 ? "99+" : String(count)));
</script>

{#if shown}
  <span class={`badge tone-${tone} ${className}`} aria-label={label}>{shown}</span>
{/if}
