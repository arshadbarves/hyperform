<script lang="ts">
  import { SCOPE_OPTIONS } from "$lib/domain/settings/scopes";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Menu } from "$lib/ui";
  import Icon, { Check, ChevronDown } from "$lib/ui/icons";

  /** Which settings file this page writes: one quiet line, the choice behind it. */
  const options = $derived(SCOPE_OPTIONS.filter((option) => settingsStore.scopes.includes(option.value)));
  const selected = $derived(SCOPE_OPTIONS.find((option) => option.value === settingsStore.scope));
  const file = $derived(settingsStore.files[settingsStore.scope]);
  const where = $derived(file ? `${file.path}${file.exists ? "" : " (created on the first change)"}` : "");
</script>

<div class="scope-menu">
  <span class="scope-menu-label">Saving to</span>
  {#if options.length > 1}
    <Menu.Root>
      <Menu.Trigger class="scope-menu-trigger" title={where}>
        {selected?.label}
        <Icon icon={ChevronDown} size={11} />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Content class="menu scope-menu-list" side="bottom" align="end" sideOffset={6}>
          {#each options as option (option.value)}
            <Menu.Item class="menu-item is-stacked" onSelect={() => (settingsStore.scope = option.value)}>
              <span class="menu-item-label">
                {option.label}
                {#if option.value === settingsStore.scope}<Icon icon={Check} size={12} />{/if}
              </span>
              <span class="menu-item-sub">{option.description}</span>
            </Menu.Item>
          {/each}
          {#if where}
            <Menu.Separator class="menu-sep" />
            <p class="scope-menu-path" title={where}>{where}</p>
          {/if}
        </Menu.Content>
      </Menu.Portal>
    </Menu.Root>
  {:else}
    <span class="scope-menu-value" title={where}>{selected?.label}</span>
  {/if}
</div>
