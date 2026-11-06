<script lang="ts">
  import { tick } from "svelte";
  import { approvalQuestion, PREVIEW_LINES, previewLines } from "$lib/domain/approvals";
  import { compactJson } from "$lib/domain/tools/toolInput";
  import { toolSubject } from "$lib/domain/tools/toolMeta";
  import type { ApprovalDecision } from "$lib/protocol/ApprovalDecision";
  import type { ApprovalRequest } from "$lib/protocol/ApprovalRequest";
  import { resolveApproval } from "$lib/runtime";
  import { Button, buttonClass, Kbd, Menu, Pill } from "$lib/ui";
  import Icon, { ChevronDown, ShieldAlert } from "$lib/ui/icons";
  import DiffRows from "../overlays/DiffRows.svelte";

  /**
   * An approval, docked in the composer: a question with the few words that
   * decide it, a short preview (unfold for all of it), Allow once, "Always
   * allow…" for rules, and Deny with optional feedback. y / s / p / n answer
   * while it has focus.
   */
  type Props = { request: ApprovalRequest; agentLabel: string | null; autoFocus?: boolean };
  let { request, agentLabel, autoFocus = false }: Props = $props();

  let root: HTMLDivElement | undefined = $state();
  let feedbackEl: HTMLInputElement | undefined = $state();
  let denying = $state(false);
  let feedback = $state("");
  let sending = $state(false);
  let expanded = $state(false);

  const preview = $derived(request.preview);
  const rule = $derived(request.suggestedRule);
  const canProject = $derived(request.canPersist && Boolean(rule));
  const fallback = $derived(toolSubject(request.tool, request.input) || compactJson(request.input, 400));
  const previewText = $derived(
    preview?.type === "diff" ? preview.diff : preview?.type === "command" ? preview.command : preview?.type === "text" ? preview.text : fallback,
  );
  const long = $derived(previewLines(previewText) > PREVIEW_LINES);

  // The card replaces the composer's draft, so it takes focus unless you are typing in another field.
  $effect(() => {
    if (!autoFocus || !root) return;
    const active = document.activeElement;
    const typingElsewhere = active instanceof HTMLElement && active.matches("input, textarea, [contenteditable]") && !root.closest(".composer")?.contains(active);
    if (!typingElsewhere) root.focus({ preventScroll: true });
  });

  async function decide(decision: ApprovalDecision) {
    if (sending) return;
    sending = true;
    if (!(await resolveApproval(request.requestId, decision))) sending = false;
  }

  async function startDeny() {
    denying = true;
    await tick();
    feedbackEl?.focus();
  }

  function deny() {
    void decide({ type: "deny", feedback: feedback.trim() || null });
  }

  function onKey(e: KeyboardEvent) {
    if (e.target !== root || e.metaKey || e.ctrlKey || e.altKey) return;
    const key = e.key.toLowerCase();
    if (key === "y") void decide({ type: "allowOnce" });
    else if (key === "s" && rule) void decide({ type: "allowSession", rule });
    else if (key === "p" && canProject && rule) void decide({ type: "allowProject", rule });
    else if (key === "n") void startDeny();
    else return;
    e.preventDefault();
  }
</script>

<!-- The card takes focus so y/s/p/n answer it without a pointer. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="approval-card"
  role="group"
  aria-label={approvalQuestion(request)}
  tabindex="0"
  data-pending-card
  bind:this={root}
  onkeydown={onKey}
>
  <header class="approval-head">
    <span class="approval-icon" aria-hidden="true"><Icon icon={ShieldAlert} size={15} /></span>
    <div class="approval-titles">
      <p class="approval-question">{approvalQuestion(request)}</p>
      {#if request.reason || agentLabel}
        <p class="approval-why">
          {#if agentLabel}<Pill tone="info">{agentLabel}</Pill>{/if}
          {#if request.reason}<span>{request.reason}</span>{/if}
        </p>
      {/if}
    </div>
  </header>

  {#if previewText}
    <div class="approval-preview" class:is-folded={long && !expanded}>
      {#if preview?.type === "diff"}
        <DiffRows text={preview.diff} path={preview.path} maxRows={120} />
      {:else}
        <pre class="approval-code"><code>{previewText}</code></pre>
      {/if}
    </div>
    {#if preview?.type === "command" && preview.description}<p class="approval-note">{preview.description}</p>{/if}
    {#if long}
      <button type="button" class="approval-more" onclick={() => (expanded = !expanded)}>
        {expanded ? "Show less" : `Show all ${previewLines(previewText)} lines`}
      </button>
    {/if}
  {/if}

  {#if denying}
    <div class="approval-feedback">
      <input
        bind:this={feedbackEl}
        bind:value={feedback}
        placeholder="Tell the agent what to do instead (optional)"
        onkeydown={(e) => {
          if (e.key === "Enter") deny();
          if (e.key === "Escape") {
            denying = false;
            root?.focus();
          }
        }}
      />
      <Button variant="danger" disabled={sending} onclick={deny}>Deny</Button>
      <Button onclick={() => (denying = false)}>Cancel</Button>
    </div>
  {:else}
    <div class="approval-actions">
      <Button variant="accent" disabled={sending} onclick={() => void decide({ type: "allowOnce" })}>Allow once</Button>
      {#if rule}
        <Menu.Root>
          <Menu.Trigger class={buttonClass("secondary")} disabled={sending}>
            <span>Always allow…</span>
            <Icon icon={ChevronDown} size={11} />
          </Menu.Trigger>
          <Menu.Portal>
            <Menu.Content class="menu" side="bottom" align="start" sideOffset={6}>
              <Menu.Item class="menu-item is-stacked" onSelect={() => void decide({ type: "allowSession", rule })}>
                <span class="menu-item-label">In this chat</span>
                <span class="menu-item-sub">Rule {rule}</span>
              </Menu.Item>
              {#if canProject}
                <Menu.Item class="menu-item is-stacked" onSelect={() => void decide({ type: "allowProject", rule })}>
                  <span class="menu-item-label">In this project</span>
                  <span class="menu-item-sub">Saved to personal settings</span>
                </Menu.Item>
              {/if}
            </Menu.Content>
          </Menu.Portal>
        </Menu.Root>
      {/if}
      <Button class="approval-deny" disabled={sending} onclick={() => void startDeny()}>Deny…</Button>
      <span class="approval-keys" aria-hidden="true">
        <Kbd keys="y" /> once
        {#if rule}<Kbd keys="s" /> chat{/if}
        {#if canProject}<Kbd keys="p" /> project{/if}
        <Kbd keys="n" /> deny
      </span>
    </div>
  {/if}
</div>
