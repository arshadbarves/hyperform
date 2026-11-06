<script lang="ts">
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import { Menu } from "$lib/ui";
  import Icon, { Rewind } from "$lib/ui/icons";

  type Props = { canRestoreCode: boolean; disabled?: boolean; onRewind: (scope: RewindScope) => void };
  let { canRestoreCode, disabled = false, onRewind }: Props = $props();

  const OPTIONS: Array<{ scope: RewindScope; label: string; sub: string; needsCode: boolean }> = [
    { scope: "both", label: "Code and conversation", sub: "Restore files and drop later messages", needsCode: true },
    { scope: "conversation", label: "Conversation only", sub: "Drop later messages; keep files as they are", needsCode: false },
    { scope: "code", label: "Code only", sub: "Restore files; keep the conversation", needsCode: true },
  ];
</script>

<Menu.Root>
  <Menu.Trigger class="turn-action" {disabled} aria-label="Rewind to before this prompt" title="Rewind">
    <Icon icon={Rewind} size={13} />
  </Menu.Trigger>
  <Menu.Portal>
    <Menu.Content class="menu" side="bottom" align="start" sideOffset={6}>
      <div class="menu-head">Rewind to before this prompt</div>
      {#each OPTIONS as option (option.scope)}
        <Menu.Item
          class="menu-item is-stacked"
          disabled={option.needsCode && !canRestoreCode}
          onSelect={() => onRewind(option.scope)}
        >
          <span class="menu-item-label">{option.label}</span>
          <span class="menu-item-sub">
            {option.needsCode && !canRestoreCode ? "No code checkpoint for this prompt" : option.sub}
          </span>
        </Menu.Item>
      {/each}
    </Menu.Content>
  </Menu.Portal>
</Menu.Root>
