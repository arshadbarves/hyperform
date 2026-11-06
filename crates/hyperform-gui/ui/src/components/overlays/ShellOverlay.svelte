<script lang="ts">
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { clearShell, hideShell, shellStore } from "$lib/shellStore";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy, Expand, Shrink, Terminal, Trash2, X } from "$lib/ui/icons";

  /**
   * The output of your own `!` commands, in a drawer that opens from the
   * top edge of the composer. Esc in the composer hides it.
   */
  const shell = bindStore(shellStore);
  const copied = copyFeedback();
  let scroller: HTMLDivElement | undefined = $state();
  let tall = $state(false);

  const entries = $derived(shell.current.entries);
  const last = $derived(entries[entries.length - 1]);
  const text = $derived(entries.map((e) => (e.cmd ? `$ ${e.cmd}\n${e.lines.join("\n")}` : e.lines.join("\n"))).join("\n\n"));

  $effect(() => {
    void entries;
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  });
</script>

{#if shell.current.visible && last}
  <section class="shell-drawer" class:is-tall={tall} aria-label="Your shell commands">
    <header class="shell-drawer-head">
      <Icon icon={Terminal} size={13} />
      <span class="shell-drawer-title">Shell</span>
      {#if last.cmd}<code class="shell-drawer-cmd" title={last.cmd}>{last.cmd}</code>{/if}
      <span class="shell-drawer-spacer"></span>
      <button type="button" class="icon-btn-mini" aria-label="Copy the output" title="Copy the output" disabled={!text} onclick={() => void copied.copy(text)}>
        <Icon icon={copied.copied ? Check : Copy} size={12} />
      </button>
      <button type="button" class="icon-btn-mini" aria-label={tall ? "Make it shorter" : "Make it taller"} title={tall ? "Shorter" : "Taller"} onclick={() => (tall = !tall)}>
        <Icon icon={tall ? Shrink : Expand} size={12} />
      </button>
      <button type="button" class="icon-btn-mini" aria-label="Clear" title="Clear" onclick={clearShell}>
        <Icon icon={Trash2} size={12} />
      </button>
      <button type="button" class="icon-btn-mini" aria-label="Hide" title="Hide (Esc)" onclick={hideShell}>
        <Icon icon={X} size={12} />
      </button>
    </header>
    <div class="shell-drawer-body" bind:this={scroller}>
      {#each entries as entry (entry.id)}
        {#if entry.cmd}<p class="shell-drawer-line is-cmd"><span aria-hidden="true">$</span> {entry.cmd}</p>{/if}
        {#if entry.lines.length}<pre class="shell-drawer-out">{entry.lines.join("\n")}</pre>{/if}
      {/each}
    </div>
  </section>
{/if}
