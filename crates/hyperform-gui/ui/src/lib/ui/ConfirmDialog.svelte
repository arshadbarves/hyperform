<script lang="ts" module>
  import { AlertDialog } from "bits-ui";
  import { buttonClass } from "./button";
</script>

<script lang="ts">
  /** A question that must be answered before something irreversible happens. */
  type Props = {
    open: boolean;
    title: string;
    description?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    tone?: "danger" | "default";
    onConfirm: () => void;
    onCancel: () => void;
  };

  let {
    open,
    title,
    description,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    tone = "default",
    onConfirm,
    onCancel,
  }: Props = $props();
</script>

<AlertDialog.Root
  {open}
  onOpenChange={(next) => {
    if (!next) onCancel();
  }}
>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="modal-overlay" />
    <AlertDialog.Content class="modal is-alert">
      <header class="modal-header">
        <div class="modal-heading">
          <AlertDialog.Title class="modal-title">{title}</AlertDialog.Title>
          {#if description}
            <AlertDialog.Description class="modal-desc">{description}</AlertDialog.Description>
          {/if}
        </div>
      </header>
      <footer class="modal-footer">
        <AlertDialog.Cancel class={buttonClass("secondary")}>{cancelLabel}</AlertDialog.Cancel>
        <AlertDialog.Action
          class={tone === "danger" ? buttonClass("danger", { solid: true }) : buttonClass("accent")}
          onclick={onConfirm}
        >
          {confirmLabel}
        </AlertDialog.Action>
      </footer>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>
