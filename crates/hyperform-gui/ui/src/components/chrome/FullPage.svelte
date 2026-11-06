<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "$lib/ui";
  import Icon, { ChevronLeft } from "$lib/ui/icons";
  import { sheet } from "$lib/ui/motion";
  import WindowControlsMaybe from "./WindowControlsMaybe.svelte";

  /**
   * A full-window page over the app (Settings, the prompt inspector): a
   * title bar on the window with Back, the title and the page's actions,
   * then its body, whose floating cards the page brings (panels.css). It
   * opens and closes as a sheet (`sheet` in motion.ts) when its
   * parent mounts or unmounts it; Esc goes back unless something inside
   * used the key.
   */
  type Props = {
    title: string;
    onClose: () => void;
    /** Beside the title, e.g. the model the inspected request went to. */
    lead?: Snippet;
    /** Right of the title bar, before the window controls. */
    actions?: Snippet;
    children: Snippet;
  };

  let { title, onClose, lead, actions, children }: Props = $props();

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      e.preventDefault();
      onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="full-page-frame" role="presentation" transition:sheet|global>
  <div class="full-page" role="dialog" tabindex="-1" aria-label={title}>
    <header class="full-page-bar" data-tauri-drag-region>
      <div class="full-page-bar-side" data-tauri-drag-region>
        <Button variant="icon" title="Back (Esc)" aria-label="Back" onclick={onClose}>
          <Icon icon={ChevronLeft} size={15} />
        </Button>
        <h1 class="full-page-title">{title}</h1>
        {@render lead?.()}
      </div>
      <div class="full-page-bar-side" data-tauri-drag-region>
        {@render actions?.()}
        <WindowControlsMaybe />
      </div>
    </header>
    <div class="full-page-body">
      {@render children()}
    </div>
  </div>
</div>
