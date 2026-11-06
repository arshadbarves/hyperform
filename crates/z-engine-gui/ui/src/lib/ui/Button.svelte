<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { buttonClass, type ButtonSize, type ButtonVariant } from "./button";

  type Props = Omit<HTMLButtonAttributes, "class"> & {
    variant?: ButtonVariant;
    size?: ButtonSize;
    solid?: boolean;
    active?: boolean;
    spinning?: boolean;
    class?: string;
    ref?: HTMLButtonElement;
  };

  let {
    variant = "ghost",
    size = "m",
    solid = false,
    active = false,
    spinning = false,
    class: className = "",
    type = "button",
    ref = $bindable(),
    children,
    ...rest
  }: Props = $props();
</script>

<button
  bind:this={ref}
  {type}
  class={buttonClass(variant, { size, solid, active, spinning, className })}
  {...rest}>{@render children?.()}</button
>
