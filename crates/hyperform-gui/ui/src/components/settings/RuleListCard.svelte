<script lang="ts">
  import { addPermissionRule, removePermissionRule } from "$lib/commands";
  import { stringList, unionItems } from "$lib/domain/settings/provenance";
  import { ruleHint } from "$lib/domain/settings/ruleSyntax";
  import { LAYER_LABELS, layerOf, scopeLabel } from "$lib/domain/settings/scopes";
  import type { RuleKind } from "$lib/protocol/config/RuleKind";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button, Pill, type PillTone } from "$lib/ui";
  import Icon, { Check, Plus, Trash2 } from "$lib/ui/icons";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  const RULE_TONES: Record<RuleKind, PillTone> = { allow: "ok", ask: "attention", deny: "danger" };

  type Props = { kind: RuleKind; title: string; description: string; presets: readonly string[] };
  let { kind, title, description, presets }: Props = $props();

  const id = $props.id();
  let draft = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const path = $derived(["permissions", kind]);
  const own = $derived(stringList(settingsStore.scopeValue(path)));
  const inherited = $derived.by(() => {
    const provenance = settingsStore.provenance;
    if (!provenance) return [];
    return unionItems(provenance, path).filter((item) => !item.scopes.includes(settingsStore.scope));
  });
  const hint = $derived(ruleHint(draft));
  /** Lists union across layers, so a rule another file lists is already in effect. */
  const present = $derived(new Set([...own, ...inherited.map((item) => item.value)]));

  async function run(op: Parameters<typeof settingsStore.write>[0]) {
    busy = true;
    error = await settingsStore.write(op);
    busy = false;
    return error === null;
  }

  async function add(rule: string) {
    const text = rule.trim();
    if (!text || own.includes(text) || ruleHint(text).level === "error") return;
    if ((await run((target) => addPermissionRule(target, kind, text))) && text === draft.trim()) draft = "";
  }
</script>

<SettingsGroup {title} {description}>
  <SettingsCard>
    {#if own.length === 0 && inherited.length === 0}
      <div class="extension-empty">No {kind} rules yet. Rules added here are saved to {scopeLabel(settingsStore.scope)} settings.</div>
    {:else}
      <div class="permission-rules-list">
        {#each own as rule (rule)}
          <div class="permission-rule-row">
            <div class="permission-rule-left">
              <code class="permission-rule-code">{rule}</code>
              <Pill tone={RULE_TONES[kind]}>{scopeLabel(settingsStore.scope)}</Pill>
            </div>
            <button
              type="button"
              class="icon-btn-mini setting-remove"
              disabled={busy}
              title={`Remove ${rule}`}
              aria-label={`Remove rule ${rule}`}
              onclick={() => void run((target) => removePermissionRule(target, kind, rule))}
            >
              <Icon icon={Trash2} size={13} />
            </button>
          </div>
        {/each}
        {#each inherited as item (item.value)}
          <div class="permission-rule-row inherited" title="Edit this rule in its own settings file">
            <div class="permission-rule-left">
              <code class="permission-rule-code">{item.value}</code>
              <Pill>{item.scopes.map((scope) => LAYER_LABELS[layerOf(scope)]).join(", ")}</Pill>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <form
      class="permission-add-form"
      onsubmit={(event) => {
        event.preventDefault();
        void add(draft);
      }}
    >
      <div class="permission-input-wrap">
        <input
          {id}
          class="setting-input mono"
          bind:value={draft}
          placeholder="e.g. Bash(npm test:*), Edit(src/**), WebFetch(domain:docs.rs)"
          spellcheck={false}
          autocomplete="off"
          aria-describedby={`${id}-hint`}
          aria-invalid={hint.level === "error"}
          oninput={() => (error = null)}
        />
        <Button type="submit" variant="secondary" disabled={busy || !draft.trim() || hint.level === "error" || own.includes(draft.trim())}>
          <Icon icon={Plus} size={13} />
          <span>Add</span>
        </Button>
      </div>
      <p id={`${id}-hint`} class={`rule-hint ${hint.level}`} aria-live="polite">
        {own.includes(draft.trim()) ? "Already in this list." : hint.message}
      </p>
      {#if error}<p class="setting-error" role="alert">{error}</p>{/if}
    </form>
  </SettingsCard>
  {#if presets.length > 0}
    <div class="permission-presets-row">
      {#each presets as rule (rule)}
        {@const added = present.has(rule)}
        <button
          type="button"
          class={`preset-chip-btn${added ? " is-added" : ""}`}
          disabled={added || busy}
          onclick={() => void add(rule)}
          title={added ? "Already in effect" : `Add ${rule}`}
        >
          <Icon icon={added ? Check : Plus} size={12} class={added ? "chip-icon-added" : undefined} />
          <code>{rule}</code>
        </button>
      {/each}
    </div>
  {/if}
</SettingsGroup>
