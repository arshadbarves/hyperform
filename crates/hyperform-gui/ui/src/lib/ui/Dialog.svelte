<script lang="ts" module>
  import { Dialog as Bits } from "bits-ui";
  export const Root = Bits.Root;
  export const Close = Bits.Close;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconSvgElement } from "./icons";

  /**
   * The dialog sheet (kit.css `.modal`): a header with an optional icon, the
   * title and a line under it, then the body. Put the actions last in a
   * `.modal-footer`, the secondary one before the primary one.
   */
  type Props = {
    title?: string;
    description?: string;
    icon?: IconSvgElement;
    /** The icon's color, such as a provider's brand. */
    iconColor?: string;
    /** Names the dialog for assistive tech when it shows no title. */
    label?: string;
    overlayClass?: string;
    contentClass?: string;
    closing?: boolean;
    children: Snippet;
  };

  let {
    title,
    description,
    icon,
    iconColor,
    label,
    overlayClass = "",
    contentClass = "",
    closing = false,
    children,
  }: Props = $props();
</script>

<Bits.Portal>
  <Bits.Overlay class="modal-overlay{closing ? ' is-closing' : ''} {overlayClass}" />
  <Bits.Content class="modal{closing ? ' is-closing' : ''} {contentClass}" aria-label={title ? undefined : label}>
    {#if title}
      <header class="modal-header">
        {#if icon}
          <span class="modal-icon" style:color={iconColor} aria-hidden="true"><Icon {icon} size={16} /></span>
        {/if}
        <div class="modal-heading">
          <Bits.Title class="modal-title">{title}</Bits.Title>
          {#if description}<Bits.Description class="modal-desc">{description}</Bits.Description>{/if}
        </div>
      </header>
    {/if}
    {@render children()}
  </Bits.Content>
</Bits.Portal>
