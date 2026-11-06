<script lang="ts" module>
  /** Context: a getter that is true while the reply around the block is still streaming. */
  export const STREAMING = Symbol("markdown-streaming");
</script>

<script lang="ts">
  import { getContext, type Snippet } from "svelte";
  import { highlightCode } from "$lib/highlight";
  import { pushToast } from "$lib/runtime";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy } from "$lib/ui/icons";
  import { whenVisible } from "$lib/ui/whenVisible";

  /**
   * A fenced code block with its language and a Copy button. Colors arrive
   * once the block nears the screen and its reply has finished streaming.
   */
  type Props = { children?: Snippet };
  let { children }: Props = $props();

  const streaming = getContext<(() => boolean) | undefined>(STREAMING) ?? (() => false);
  const feedback = copyFeedback();
  let pre: HTMLPreElement | undefined = $state();
  let lang = $state("");
  let html = $state<string | null>(null);

  $effect(() => {
    const el = pre;
    if (!el) return;
    const found = el.querySelector("code")?.className.match(/language-([\w+-]+)/)?.[1] ?? "";
    lang = found;
    if (!found || streaming()) return;
    return whenVisible(el, () => {
      html = highlightCode(el.textContent ?? "", found);
    });
  });

  async function copy() {
    if (!(await feedback.copy(pre?.textContent ?? ""))) pushToast("Copy failed", "warn");
  }
</script>

<div class="code-block">
  <div class="code-block-head">
    <span class="code-lang">{lang || "code"}</span>
    <button type="button" class="code-copy-btn" class:is-copied={feedback.copied} aria-label="Copy code" onclick={() => void copy()}>
      <Icon icon={feedback.copied ? Check : Copy} size={11} />
      <span>{feedback.copied ? "Copied" : "Copy"}</span>
    </button>
  </div>
  <pre bind:this={pre}>{#if html !== null}<code class={`language-${lang} hljs`}>{@html html}</code>{:else}{@render children?.()}{/if}</pre>
</div>
