<script lang="ts">
  import { contextBreakdown } from "$lib/commands";
  import type { CtxMeter } from "$lib/domain/contextMeter";
  import { compact, pushToast } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button } from "$lib/ui";
  import { fmtTokens } from "$lib/util";

  /** What fills the context window, layer by layer, with the two things to do about it. */
  type Props = { sessionId: string; meter: CtxMeter; busy: boolean; onClose: () => void };
  let { sessionId, meter, busy, onClose }: Props = $props();

  let loading = $state(true);
  let compacting = $state(false);
  const compactAt = $derived(settingsStore.settings?.context.compact_at_percent ?? null);

  $effect(() => {
    const id = sessionId;
    loading = true;
    void contextBreakdown(id)
      .catch(() => null)
      .finally(() => (loading = false));
  });

  function share(tokens: number): string {
    return `${Math.max(0, Math.round((tokens / meter.max) * 100))}%`;
  }

  async function onCompact() {
    compacting = true;
    if (await compact()) {
      pushToast("Compacting the conversation…", "info");
      onClose();
    }
    compacting = false;
  }

  function onInspect() {
    onClose();
    ui.openPanel("context");
  }
</script>

<div class="context-card-body">
  <header class="context-head">
    <p class="context-title">Context</p>
    <p class={`context-pct level-${meter.level}`}>{meter.pct}% used</p>
    <p class="context-sub">
      {fmtTokens(meter.used)} of {fmtTokens(meter.max)} tokens · {fmtTokens(meter.remaining)} left
    </p>
    <div class="context-bar" class:is-loading={loading && meter.totalOnly} aria-hidden="true">
      {#each meter.slices as slice (slice.id)}
        <span style:width={share(slice.tokens)} style:background={slice.color}></span>
      {/each}
    </div>
  </header>

  {#if !meter.totalOnly || !loading || compactAt}
    <section class="context-section">
      {#if !meter.totalOnly}
        <ul class="context-legend">
          {#each meter.slices as slice (slice.id)}
            <li>
              <span class="context-swatch" style:background={slice.color}></span>
              <span class="context-layer">{slice.label}</span>
              <span class="context-tokens">{fmtTokens(slice.tokens)}</span>
            </li>
          {/each}
        </ul>
      {:else if !loading}
        <p class="context-note">The breakdown appears after the first reply.</p>
      {/if}
      {#if compactAt}
        <p class="context-note">HyperForm compacts older messages by itself at {compactAt}%.</p>
      {/if}
    </section>
  {/if}

  <section class="context-section context-actions">
    <Button variant="secondary" size="s" disabled={compacting || busy} onclick={onCompact}>Compact now</Button>
    <Button size="s" onclick={onInspect}>Inspect prompt</Button>
  </section>
</div>
