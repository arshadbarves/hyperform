<script lang="ts">
  import { setTrust } from "$lib/commands";
  import { errorText } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { ShieldAlert } from "$lib/ui/icons";

  /** What of the project waits for trust, e.g. "hooks". */
  type Props = { what: string };
  let { what }: Props = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);
  const trust = $derived(settingsStore.trust);
  const shown = $derived(
    Boolean(settingsStore.root && trust && !trust.trusted && (trust.projectDefines.length > 0 || settingsStore.scope !== "user")),
  );

  async function grant() {
    const root = settingsStore.root;
    if (!root) return;
    busy = true;
    try {
      await setTrust(root, true);
      error = null;
    } catch (e) {
      error = errorText(e);
    }
    await settingsStore.loadTrust();
    busy = false;
  }
</script>

{#if shown}
  <div class="trust-note" role="status">
    <Icon icon={ShieldAlert} size={15} />
    <div class="trust-note-copy">
      <strong>This workspace is not trusted</strong>
      <span>
        Project {what} do not run until you trust it.
        {#if trust?.projectDefines.length}It defines {trust.projectDefines.join(", ")}.{/if}
      </span>
      {#if error}<span class="setting-error">{error}</span>{/if}
    </div>
    <button type="button" class="btn-secondary" disabled={busy} onclick={() => void grant()}>Trust workspace</button>
  </div>
{/if}
