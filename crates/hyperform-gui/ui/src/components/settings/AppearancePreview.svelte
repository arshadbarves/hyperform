<script lang="ts">
  import type { TaskReportView } from "$lib/protocol/config/TaskReportView";
  import SettingsCard from "./SettingsCard.svelte";

  type Props = { view: TaskReportView; label: string };
  let { view, label }: Props = $props();
</script>

<SettingsCard class="appearance-preview-card">
  <div class="preview-head">
    <span class="preview-eyebrow">Completed task report</span>
    <span class="preview-mode">{label}</span>
  </div>
  <div class="preview-result">
    <span class="preview-status-dot"></span>
    <span class="preview-line preview-line-strong"></span>
  </div>
  {#if view !== "quiet"}
    <div class="preview-summary">
      <span class="preview-line"></span>
      <span class="preview-count">Checks</span>
    </div>
  {/if}
  {#if view === "detailed"}
    <div class="preview-detail">
      <span class="preview-detail-label">Verification</span>
      <span class="preview-line"></span>
      <span class="preview-line preview-line-short"></span>
    </div>
    <div class="preview-detail">
      <span class="preview-detail-label">Evidence</span>
      <span class="preview-line"></span>
    </div>
  {:else}
    <div class="preview-disclosure">Supporting details</div>
  {/if}
</SettingsCard>

<style>
  :global(.appearance-preview-card) {
    padding: 16px;
  }

  .preview-head,
  .preview-result,
  .preview-summary {
    display: flex;
    align-items: center;
  }

  .preview-head {
    justify-content: space-between;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--separator);
  }

  .preview-eyebrow,
  .preview-mode,
  .preview-count,
  .preview-detail-label,
  .preview-disclosure {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .preview-eyebrow,
  .preview-detail-label,
  .preview-disclosure {
    color: var(--label-3);
  }

  .preview-mode {
    color: var(--label);
    padding: 2px 7px;
    border: 1px solid var(--separator);
    border-radius: 999px;
  }

  .preview-result {
    gap: 9px;
    padding: 15px 0 10px;
  }

  .preview-status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--tone-working);
    box-shadow: 0 0 0 3px var(--tone-working-soft);
  }

  .preview-line {
    display: block;
    width: 46%;
    height: 6px;
    border-radius: 999px;
    background: var(--raised-3);
  }

  .preview-line-strong {
    width: 62%;
    background: var(--label-2);
    opacity: 0.55;
  }

  .preview-line-short {
    width: 31%;
  }

  .preview-summary {
    justify-content: space-between;
    padding: 8px 10px;
    background: var(--fill-quiet);
    border-radius: var(--r-s);
  }

  .preview-count {
    color: var(--label-2);
  }

  .preview-detail {
    display: grid;
    grid-template-columns: 80px 1fr;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    padding: 8px 10px;
    border-top: 1px solid var(--separator);
  }

  .preview-detail .preview-line-short {
    grid-column: 2;
  }

  .preview-disclosure {
    margin-top: 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--separator);
  }
</style>
