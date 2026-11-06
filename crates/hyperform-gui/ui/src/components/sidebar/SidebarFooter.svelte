<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { openSettings } from "$lib/stores/app-actions";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { Kbd } from "$lib/ui";
  import Icon, { Settings } from "$lib/ui/icons";
  import { perch } from "$lib/ui/perch.svelte";
  import { updateStore } from "$lib/updateStore";

  /** Settings and a waiting update. The roaming pet likes to stand on this edge. */
  const mod = modLabel();
  const update = bindStore(updateStore);
</script>

<div class="sidebar-footer" use:perch={{ id: "sidebar", kind: "edge" }}>
  <button type="button" class="sidebar-row" onclick={() => openSettings()}>
    <Icon icon={Settings} size={14} strokeWidth={1.8} />
    <span class="sidebar-row-label">Settings</span>
    <Kbd keys={`${mod},`} />
  </button>
  {#if update.current.info?.available}
    <button type="button" class="update-pill" onclick={() => openSettings("about")}>Update</button>
  {/if}
</div>
