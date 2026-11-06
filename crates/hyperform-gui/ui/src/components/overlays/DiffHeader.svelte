<script lang="ts">
  import type { FileSummary } from "$lib/domain/diffRows";
  import { plural } from "$lib/domain/format";
  import { ui, type DiffLayout, type DiffScope } from "$lib/stores/ui.svelte";
  import { SegmentedChoice } from "$lib/ui";
  import Icon, { PanelLeft, RefreshCw } from "$lib/ui/icons";

  /** The Changes tab's toolbar: the file list, how many changed, which changes, the layout, refresh. */
  type Props = {
    scope: DiffScope;
    summary: FileSummary;
    refreshing: boolean;
    filesOpen: boolean;
    onScope: (scope: DiffScope) => void;
    onRefresh: () => void;
    onToggleFiles: () => void;
  };
  let { scope, summary, refreshing, filesOpen, onScope, onRefresh, onToggleFiles }: Props = $props();

  const SCOPES = [
    { value: "session", label: "This chat", description: "Only the files this chat changed" },
    { value: "git", label: "Uncommitted", description: "Every uncommitted change in the project, against HEAD" },
  ] as const;
  const LAYOUTS = [
    { value: "unified", label: "Unified", description: "Old and new lines in one column" },
    { value: "split", label: "Split", description: "Old on the left, new on the right" },
  ] as const;
</script>

<header class="diff-head">
  <div class="diff-head-title">
    {#if summary.files > 1}
      <button type="button" class="icon-btn" class:is-active={filesOpen} aria-label={filesOpen ? "Hide the file list" : "Show the file list"} aria-pressed={filesOpen} onclick={onToggleFiles}>
        <Icon icon={PanelLeft} size={14} />
      </button>
    {/if}
    {#if summary.files > 0}
      <span class="diff-head-sub">
        {plural(summary.files, "file")}
        {#if summary.added}<span class="diff-add-text">+{summary.added}</span>{/if}
        {#if summary.removed}<span class="diff-del-text">−{summary.removed}</span>{/if}
      </span>
    {/if}
  </div>
  <div class="diff-head-actions">
    <SegmentedChoice compact label="Which changes" options={SCOPES} value={scope} onSelect={onScope} />
    <div class="diff-layout-choice">
      <SegmentedChoice
        compact
        label="Layout"
        options={LAYOUTS}
        value={ui.diffLayout}
        onSelect={(layout: DiffLayout) => ui.setDiffLayout(layout)}
      />
    </div>
    <button type="button" class="icon-btn" class:spinning={refreshing} aria-label="Refresh" title="Refresh" onclick={onRefresh}>
      <Icon icon={RefreshCw} size={14} />
    </button>
  </div>
</header>
