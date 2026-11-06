<script lang="ts" generics="T extends string">
  import type { ChoiceOption } from "$lib/domain/settings/options";
  import type { KeyPath } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { SegmentedChoice } from "$lib/ui";
  import SettingRow from "./SettingRow.svelte";

  type Props = {
    title: string;
    description?: string;
    keyPath: KeyPath;
    options: readonly ChoiceOption<T>[];
    value: T;
    /** The stored value of a choice; null removes the key. Defaults to the choice itself. */
    toValue?: (choice: T) => unknown;
    tag?: string;
  };

  let { title, description, keyPath, options, value, toValue = (choice) => choice, tag }: Props = $props();

  let pending = $state<T | null>(null);
  let error = $state<string | null>(null);
  const shown = $derived(pending ?? value);
  const selected = $derived(options.find((option) => option.value === shown));

  async function pick(choice: T) {
    if (pending) return;
    pending = choice;
    error = await settingsStore.setValue(keyPath, toValue(choice));
    pending = null;
  }
</script>

<SettingRow {title} {description} {keyPath} {error} {tag}>
  <SegmentedChoice label={title} {options} value={shown} busy={pending !== null} onSelect={(choice) => void pick(choice)} />
  {#if selected}<p class="setting-choice-desc" aria-live="polite">{selected.description}</p>{/if}
</SettingRow>
