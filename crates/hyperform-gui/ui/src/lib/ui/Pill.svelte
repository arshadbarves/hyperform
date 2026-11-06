<script lang="ts" module>
  /** What a pill's color means, from the status tones. */
  export type PillTone = "neutral" | "working" | "attention" | "danger" | "ok" | "info" | "shell";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  /** A short status word or tag; one vocabulary for tools, agents, jobs and interactions. */
  type Props = {
    tone?: PillTone;
    /** A leading dot; `live` makes it pulse while something runs. */
    dot?: boolean;
    live?: boolean;
    /** Just the dot and the word, without the pill around it. */
    plain?: boolean;
    title?: string;
    class?: string;
    children: Snippet;
  };

  let { tone = "neutral", dot = false, live = false, plain = false, title, class: className = "", children }: Props =
    $props();
</script>

<span class={`pill tone-${tone}${live ? " is-live" : ""}${plain ? " is-plain" : ""} ${className}`} {title}>
  {#if dot || live}<span class="pill-dot" aria-hidden="true"></span>{/if}
  {@render children()}
</span>
