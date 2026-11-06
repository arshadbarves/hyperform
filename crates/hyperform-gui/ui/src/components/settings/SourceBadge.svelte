<script lang="ts">
  import { LAYER_LABELS } from "$lib/domain/settings/scopes";
  import type { LayerScope } from "$lib/protocol/config/LayerScope";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Pill } from "$lib/ui";

  type Props = { source: LayerScope; prefix?: string };
  let { source, prefix = "" }: Props = $props();

  const path = $derived(settingsStore.loaded?.layers.find((layer) => layer.scope === source)?.path ?? null);
  const title = $derived(
    source === "default"
      ? "Built-in default: no settings file sets this"
      : source === "env"
        ? "Set by a HYPERFORM_* environment variable"
        : `From ${path ?? LAYER_LABELS[source]}`,
  );
</script>

<Pill tone={source === "env" ? "attention" : "neutral"} {title}>{prefix}{LAYER_LABELS[source]}</Pill>
