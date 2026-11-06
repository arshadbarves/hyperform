<script lang="ts">
  import { isWinPlatform } from "$lib/platform";
  import WindowControls from "./WindowControls.svelte";
</script>

{#if isWinPlatform()}
  <WindowControls />
{/if}
