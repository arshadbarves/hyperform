<script lang="ts">
  import type { IconSvgElement } from "./icons";

  type Props = {
    icon: IconSvgElement;
    size?: number;
    strokeWidth?: number;
    class?: string;
  };

  let { icon, size = 16, strokeWidth = 1.5, class: className }: Props = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  aria-hidden="true"
>
  {#each icon as node}
    {@const [tag, attrs] = node}
    {#if tag === "path"}
      <path
        {...attrs}
        stroke="currentColor"
        stroke-width={attrs["stroke-width"] ?? strokeWidth}
        stroke-linecap={(attrs["stroke-linecap"] as "round" | "butt" | "square" | undefined) ?? "round"}
        stroke-linejoin={(attrs["stroke-linejoin"] as "round" | "inherit" | "miter" | "bevel" | undefined) ?? "round"}
      />
    {:else if tag === "circle"}
      <circle {...attrs} stroke="currentColor" stroke-width={attrs["stroke-width"] ?? strokeWidth} />
    {:else if tag === "rect"}
      <rect {...attrs} stroke="currentColor" stroke-width={attrs["stroke-width"] ?? strokeWidth} />
    {:else if tag === "line"}
      <line {...attrs} stroke="currentColor" stroke-width={attrs["stroke-width"] ?? strokeWidth} />
    {:else if tag === "polyline"}
      <polyline {...attrs} stroke="currentColor" stroke-width={attrs["stroke-width"] ?? strokeWidth} />
    {:else if tag === "polygon"}
      <polygon {...attrs} stroke="currentColor" stroke-width={attrs["stroke-width"] ?? strokeWidth} />
    {/if}
  {/each}
</svg>
