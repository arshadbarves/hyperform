<script lang="ts">
  import {
    cancelDecisionModelDownload,
    decisionModelStatus,
    downloadDecisionModel,
    removeDecisionModel,
    type DecisionModelStatus,
  } from "$lib/commands";
  import { nativeModelView } from "$lib/domain/settings/nativeModel";
  import { errorText } from "$lib/runtime/toasts";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button, ProgressRing } from "$lib/ui";
  import SettingRow from "./SettingRow.svelte";

  /** The native runtime's model for `checkpoint`: downloaded only from here, with progress. */
  type Props = { checkpoint: string };
  let { checkpoint }: Props = $props();

  const POLL_MS = 500;
  let status = $state<DecisionModelStatus | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  const view = $derived(status ? nativeModelView(status) : null);

  async function refresh() {
    try {
      status = await decisionModelStatus(settingsStore.root);
    } catch (e) {
      error = errorText(e);
    }
  }

  $effect(() => {
    void checkpoint;
    void settingsStore.root;
    void refresh();
  });

  $effect(() => {
    if (!status?.downloading) return;
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(timer);
  });

  async function act(run: () => Promise<DecisionModelStatus | void>) {
    busy = true;
    error = null;
    try {
      const next = await run();
      if (next) status = next;
      else await refresh();
    } catch (e) {
      error = errorText(e);
    }
    busy = false;
  }

  async function remove() {
    const ok = await confirmStore.ask({
      title: "Remove the decision model?",
      description: `Deletes its ${status?.dir ? `folder ${status.dir}` : "files"}; downloading it again takes a while.`,
      confirmLabel: "Remove",
      tone: "danger",
    });
    if (ok) await act(() => removeDecisionModel(settingsStore.root));
  }
</script>

<SettingRow
  title="Native model"
  description="The checkpoint's model files for the in-app runtime, from a pinned revision. Downloaded only when you ask, and checked before use."
  {error}
>
  <div class="native-model">
    {#if view}
      <div class="native-model-status">
        {#if view.progress !== null}
          <ProgressRing value={view.progress} size={16} tone="working" label={`${Math.floor(view.progress * 100)}% downloaded`} />
        {/if}
        <p class={`native-model-text tone-${view.tone}`} role="status">{view.text}</p>
      </div>
      {#if view.action === "download"}
        <Button variant="secondary" size="s" disabled={busy} onclick={() => void act(() => downloadDecisionModel(settingsStore.root))}>
          {view.actionLabel}
        </Button>
      {:else if view.action === "cancel"}
        <Button variant="secondary" size="s" disabled={busy} onclick={() => void act(cancelDecisionModelDownload)}>Cancel</Button>
      {:else if view.action === "remove"}
        <Button variant="ghost" size="s" disabled={busy} onclick={() => void remove()}>Remove</Button>
      {/if}
    {:else}
      <p class="native-model-text tone-quiet" role="status">Checking…</p>
    {/if}
  </div>
</SettingRow>
