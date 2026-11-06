<script lang="ts">
  import { settingsNotices } from "$lib/domain/settings/notices";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { AlertOctagon, AlertTriangle, Info, RefreshCw } from "$lib/ui/icons";

  const COLLAPSED = 3;
  const TONE_ICONS = { error: AlertOctagon, warn: AlertTriangle, info: Info };

  let expanded = $state(false);
  const notices = $derived(settingsNotices(settingsStore.loaded));
  const visible = $derived(expanded ? notices : notices.slice(0, COLLAPSED));
</script>

{#if settingsStore.loadError || notices.length > 0}
  <div class="settings-notices" role="status">
    {#if settingsStore.loadError}
      <div class="settings-notice error">
        <Icon icon={AlertOctagon} size={13} />
        <span>Settings could not be loaded: {settingsStore.loadError}</span>
        <button type="button" class="settings-notice-action" onclick={() => void settingsStore.refresh()}>
          <Icon icon={RefreshCw} size={11} />
          <span>Retry</span>
        </button>
      </div>
    {/if}
    {#each visible as notice, index (index)}
      <div class={`settings-notice ${notice.tone}`}>
        <Icon icon={TONE_ICONS[notice.tone]} size={13} />
        <span>{notice.text}</span>
      </div>
    {/each}
    {#if notices.length > COLLAPSED}
      <button type="button" class="settings-notices-more" onclick={() => (expanded = !expanded)}>
        {expanded ? "Show fewer" : `Show ${notices.length - COLLAPSED} more`}
      </button>
    {/if}
  </div>
{/if}
