<script lang="ts">
  import type { Snippet } from "svelte";
  import { fmtDuration } from "$lib/domain/format";
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import Icon, { ChevronRight, type IconSvgElement } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";

  type Props = {
    call: ToolCallState;
    icon: IconSvgElement;
    label: string;
    subject?: string;
    badge?: string;
    extra?: string;
    expandable?: boolean;
    open?: boolean;
    /** A glimpse shown under the row while it is folded (a live output tail). */
    peek?: Snippet;
    children?: Snippet;
  };

  let {
    call,
    icon,
    label,
    subject = "",
    badge,
    extra,
    expandable = false,
    open = $bindable(false),
    peek,
    children,
  }: Props = $props();

  const STATUS_CLASS = {
    running: "running",
    ok: "ok",
    error: "bad",
    denied: "bad denied",
    cancelled: "cancelled",
  } as const;

  const running = $derived(call.status === "running");
  const clock = ticker(() => running);
  const meta = $derived(
    running
      ? call.startedAt
        ? fmtDuration(Math.max(0, clock.now - call.startedAt))
        : "…"
      : fmtDuration(call.durationMs),
  );
</script>

<div class={`msg tool-card ${STATUS_CLASS[call.status]}${expandable ? " expandable" : ""}`}>
  <button
    type="button"
    class="tool-row"
    disabled={!expandable}
    aria-expanded={expandable ? open : undefined}
    title={call.title || undefined}
    onclick={() => (open = !open)}
  >
    <span class="tool-dot" aria-hidden="true"><span class="tool-dot-inner"></span></span>
    <Icon {icon} size={12} class="tool-kind-icon" />
    <span class="tool-label">{label}</span>
    {#if badge}<span class="tool-badge">{badge}</span>{/if}
    <span class="tool-arg">{subject}</span>
    {#if extra}<span class="tool-extra">{extra}</span>{/if}
    {#if call.status === "denied" || call.status === "cancelled"}
      <span class="tool-state-tag">{call.status}</span>
    {/if}
    <span class="tool-elapsed">{meta}</span>
    {#if expandable}
      <span class={`tool-chevron${open ? " open" : ""}`} aria-hidden="true">
        <Icon icon={ChevronRight} size={11} />
      </span>
    {/if}
  </button>
  {#if open && children}
    <div class="tool-body">{@render children()}</div>
  {:else if peek}
    {@render peek()}
  {/if}
</div>
