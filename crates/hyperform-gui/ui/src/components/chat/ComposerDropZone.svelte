<script lang="ts">
  import Icon, { ImageIcon } from "$lib/ui/icons";

  /** Shown over the composer while images are dragged onto it. */
  type Props = { active: boolean };
  let { active }: Props = $props();
</script>

{#if active}
  <div class="composer-drop" aria-hidden="true">
    <Icon icon={ImageIcon} size={16} />
    <span>Drop images to attach them</span>
  </div>
{/if}
