<script lang="ts">
  import { catalogForPicker, catalogStore, fmtLimit, groupModels, isFreeModel, lookupModel } from "$lib/catalog";
  import { detectProviderId, PROVIDERS } from "$lib/providers";
  import { activeProjectRoot, sessions, setModel } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { Pill, Popover } from "$lib/ui";
  import Icon, { Brain, Check, ChevronDown, Search, X } from "$lib/ui/icons";
  import { shortModel } from "$lib/util";
  import EffortRow from "./EffortRow.svelte";

  /** The model chip: which model answers (and how hard it thinks); the picker searches the catalog. */
  const catalog = bindStore(catalogStore);
  const provider = $derived(settingsStore.activeProvider);
  const current = $derived(sessions.active?.model || settingsStore.settings?.model.main || "");
  const effort = $derived(sessions.active?.effort ?? null);
  const reasons = $derived(Boolean(effort) || Boolean(lookupModel(catalog.current, current)?.model.reasoning));
  let open = $state(false);
  let custom = $state("");
  let query = $state("");

  const groups = $derived(groupModels(catalogForPicker(catalog.current, provider?.baseUrl, provider?.hasKey ?? false), query));
  const providerName = $derived(
    PROVIDERS.find((preset) => preset.id === detectProviderId(provider?.baseUrl))?.name ?? "the active provider",
  );

  $effect(() => {
    if (!open) return;
    void catalogStore.ensure();
    void settingsStore.loadCredentials();
    void settingsStore.ensure(activeProjectRoot());
  });

  async function pick(id: string) {
    open = false;
    query = "";
    if (id !== current) await setModel(id);
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger class="composer-chip is-model" title="Switch model">
    <span>{shortModel(current) || "model"}</span>
    {#if effort}<span class="composer-chip-sub">· {effort}</span>{/if}
    <Icon icon={ChevronDown} size={10} strokeWidth={2.2} />
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content class="chip-pop model-pop glass-strong" side="top" align="end" sideOffset={8} collisionPadding={12}>
      {#if reasons}<EffortRow {effort} />{/if}
      <div class="model-search">
        <Icon icon={Search} size={12} />
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={query} placeholder="Search models or providers…" spellcheck={false} autofocus />
        {#if query}
          <button type="button" class="model-search-clear" onclick={() => (query = "")} aria-label="Clear the search">
            <Icon icon={X} size={11} />
          </button>
        {/if}
      </div>
      <div class="model-list">
        {#if groups.length === 0}
          <p class="model-empty">
            {query ? `No models match “${query}”.` : catalog.current ? `No ${providerName} models available. Check Settings.` : "Loading the catalog…"}
          </p>
        {/if}
        {#each groups as group (group.provider)}
          <p class="model-group">{group.provider}</p>
          {#each group.items as m (m.id)}
            <button type="button" class={`model-row${m.id === current ? " is-current" : ""}`} onclick={() => void pick(m.id)}>
              <span class="model-row-text">
                <span class="model-row-name">
                  {m.name}
                  {#if isFreeModel(m)}<Pill tone="ok">Free</Pill>{/if}
                  {#if m.reasoning}<Icon icon={Brain} size={10} class="model-row-reason" />{/if}
                </span>
                <span class="model-row-id">{m.id}</span>
              </span>
              {#if m.contextWindow || m.maxOutput}
                <span class="model-row-spec">{[fmtLimit(m.contextWindow), fmtLimit(m.maxOutput)].filter(Boolean).join(" / ")}</span>
              {/if}
              {#if m.id === current}<Icon icon={Check} size={13} strokeWidth={2.2} />{/if}
            </button>
          {/each}
        {/each}
      </div>
      <form
        class="model-custom"
        onsubmit={(e) => {
          e.preventDefault();
          const id = custom.trim();
          if (id) void pick(id);
          custom = "";
        }}
      >
        <input bind:value={custom} placeholder="Another model id, e.g. anthropic/claude-sonnet-4.5" spellcheck={false} />
        <button type="submit" class="btn-secondary" disabled={!custom.trim()}>Use</button>
      </form>
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
