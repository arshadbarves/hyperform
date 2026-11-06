<script lang="ts">
  import { highlightLine } from "$lib/highlight";

  /** One line of code, syntax-colored when the language is known. */
  type Props = { text: string; lang: string | null };
  let { text, lang }: Props = $props();

  const html = $derived(text ? highlightLine(text, lang) : null);
</script>

<span class="diff-code hl">
  {#if html}{@html html}{:else}{text || "\u00a0"}{/if}
</span>
