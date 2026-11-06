<script lang="ts">
  import { REMEMBER_TARGETS, type RememberScope } from "$lib/domain/remember";
  import Icon, { Bookmark } from "$lib/ui/icons";

  type Props = {
    selected: number;
    onPick: (scope: RememberScope) => void;
    onHover: (index: number) => void;
  };
  let { selected, onPick, onHover }: Props = $props();
</script>

<div class="composer-pop command-pop remember-pop glass-strong" role="listbox" aria-label="Remember in">
  <div class="cmd-pop-header">
    <div class="cmd-pop-header-title">
      <Icon icon={Bookmark} size={13} class="cmd-glow-icon" />
      <span>Remember in…</span>
    </div>
  </div>
  <div class="cmd-pop-list">
    {#each REMEMBER_TARGETS as target, i (target.scope)}
      <button
        type="button"
        role="option"
        aria-selected={i === selected}
        class={`cmd-pop-item cmd-notes${i === selected ? " sel" : ""}`}
        onmouseenter={() => onHover(i)}
        onclick={() => onPick(target.scope)}
      >
        <div class="cmd-icon-box"><Icon icon={Bookmark} size={13} strokeWidth={1.8} /></div>
        <div class="cmd-info-col">
          <div class="cmd-title-row"><span class="cmd-name">{target.label}</span></div>
          <span class="cmd-desc">{target.file}</span>
        </div>
        {#if i === selected}<div class="cmd-enter-pill"><kbd>↵</kbd></div>{/if}
      </button>
    {/each}
  </div>
  <div class="cmd-pop-footer">
    <div class="cmd-footer-left"><span class="footer-hint"><kbd>↵</kbd> save memory</span></div>
    <span class="footer-hint"><kbd>Esc</kbd> send as text</span>
  </div>
</div>
