<script lang="ts">
  import Icon, { ArrowDown } from "$lib/ui/icons";

  type Props = { onJump: () => void };
  let { onJump }: Props = $props();
</script>

<button type="button" class="jump-latest" title="Scroll to bottom" aria-label="Scroll to bottom" onclick={onJump}>
  <span class="jump-latest-inner">
    <span class="jump-latest-icon">
      <Icon icon={ArrowDown} size={13} strokeWidth={2.2} />
    </span>
    <span class="jump-latest-label">Scroll to bottom</span>
  </span>
</button>
