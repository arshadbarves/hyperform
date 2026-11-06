<script lang="ts">
  import { setHooks } from "$lib/commands";
  import { emptyHook, hookForEvent, layerHooks, type HookEventMeta } from "$lib/domain/settings/hooks";
  import { appendItem, moveItem, removeItem, replaceItem } from "$lib/domain/settings/listEdits";
  import { layerActive } from "$lib/domain/settings/provenance";
  import { layerOf } from "$lib/domain/settings/scopes";
  import type { HookConfig } from "$lib/protocol/config/HookConfig";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { ArrowDown, ArrowUp, Pencil, Plus, Trash2 } from "$lib/ui/icons";
  import HookForm from "./HookForm.svelte";
  import SourceBadge from "./SourceBadge.svelte";

  type Props = { event: HookEventMeta };
  let { event }: Props = $props();

  let editing = $state<number | "new" | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const layers = $derived(
    settingsStore.scopes.map((scope) => ({
      scope,
      hooks: layerHooks(settingsStore.files[scope]?.raw, event.name),
      applied: settingsStore.provenance ? layerActive(settingsStore.provenance, layerOf(scope)) : false,
    })),
  );
  const own = $derived(layers.find((layer) => layer.scope === settingsStore.scope)?.hooks ?? []);
  const total = $derived(layers.reduce((sum, layer) => sum + layer.hooks.length, 0));

  async function save(list: HookConfig[]): Promise<boolean> {
    busy = true;
    const hooks = list.map((hook) => hookForEvent(hook, event));
    error = await settingsStore.write((target) => setHooks(target, event.name, hooks));
    busy = false;
    return error === null;
  }
</script>

<div class="hook-event">
  <div class="hook-event-head">
    <div class="hook-event-copy">
      <strong>{event.name}</strong>
      <span>{event.description}</span>
    </div>
    <button type="button" class="btn-secondary size-s" disabled={busy || editing !== null} onclick={() => (editing = "new")}>
      <Icon icon={Plus} size={12} />
      <span>Add</span>
    </button>
  </div>

  {#if total > 0}
    <ol class="hook-list">
      {#each layers as layer (layer.scope)}
        {#each layer.hooks as hook, index (`${layer.scope}-${index}`)}
          {@const mine = layer.scope === settingsStore.scope}
          <li class="hook-row" class:inherited={!mine} class:skipped={!layer.applied}>
            {#if mine && editing === index}
              <HookForm
                {hook}
                {event}
                onSave={(next) => save(replaceItem(own, index, next))}
                onCancel={() => (editing = null)}
              />
            {:else}
              <div class="hook-row-main">
                {#if event.matcher}<code class="hook-matcher" title={event.matcher.label}>{hook.matcher ?? event.matcher.any}</code>{/if}
                <code class="hook-command" title={hook.command}>{hook.command}</code>
                <span class="hook-timeout">{hook.timeout_secs}s</span>
              </div>
              <div class="hook-row-meta">
                <SourceBadge source={layerOf(layer.scope)} />
                {#if !layer.applied}<span class="hook-skipped">not applied</span>{/if}
                {#if mine}
                  <button type="button" class="icon-btn-mini" disabled={busy || index === 0} aria-label="Run earlier" onclick={() => void save(moveItem(own, index, -1))}>
                    <Icon icon={ArrowUp} size={11} />
                  </button>
                  <button
                    type="button"
                    class="icon-btn-mini"
                    disabled={busy || index === own.length - 1}
                    aria-label="Run later"
                    onclick={() => void save(moveItem(own, index, 1))}
                  >
                    <Icon icon={ArrowDown} size={11} />
                  </button>
                  <button type="button" class="icon-btn-mini" disabled={busy || editing !== null} aria-label="Edit hook" onclick={() => (editing = index)}>
                    <Icon icon={Pencil} size={11} />
                  </button>
                  <button type="button" class="icon-btn-mini setting-remove" disabled={busy} aria-label="Remove hook" onclick={() => void save(removeItem(own, index))}>
                    <Icon icon={Trash2} size={12} />
                  </button>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      {/each}
    </ol>
  {/if}

  {#if editing === "new"}
    <HookForm
      hook={emptyHook()}
      {event}
      onSave={(next) => save(appendItem(own, next))}
      onCancel={() => (editing = null)}
    />
  {/if}
  {#if error}<p class="setting-error" role="alert">{error}</p>{/if}
</div>
