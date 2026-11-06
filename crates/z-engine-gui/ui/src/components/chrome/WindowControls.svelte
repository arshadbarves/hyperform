<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let maximized = $state(false);

  $effect(() => {
    let live = true;
    const win = getCurrentWindow();
    void win.isMaximized().then((v) => {
      if (live) maximized = v;
    });
    const unlisten = win.onResized(() => {
      void win.isMaximized().then((v) => {
        if (live) maximized = v;
      });
    });
    return () => {
      live = false;
      void unlisten.then((fn) => fn());
    };
  });
</script>

<div class="win-controls" aria-label="Window">
  <button
    type="button"
    class="win-ctrl"
    title="Minimize"
    aria-label="Minimize"
    onclick={() => void getCurrentWindow().minimize()}
  >
    <svg viewBox="0 0 10 10" aria-hidden="true">
      <rect x="1" y="4.5" width="8" height="1" fill="currentColor" />
    </svg>
  </button>
  <button
    type="button"
    class="win-ctrl"
    title={maximized ? "Restore" : "Maximize"}
    aria-label={maximized ? "Restore" : "Maximize"}
    onclick={() => void getCurrentWindow().toggleMaximize()}
  >
    {#if maximized}
      <svg viewBox="0 0 10 10" aria-hidden="true">
        <path
          d="M3 1h6v6H3V1zm1 1v4h4V2H4zm2 2h4v4H6V4z"
          fill="currentColor"
        />
      </svg>
    {:else}
      <svg viewBox="0 0 10 10" aria-hidden="true">
        <rect
          x="1.5"
          y="1.5"
          width="7"
          height="7"
          fill="none"
          stroke="currentColor"
          stroke-width="1"
        />
      </svg>
    {/if}
  </button>
  <button
    type="button"
    class="win-ctrl close"
    title="Close"
    aria-label="Close"
    onclick={() => void getCurrentWindow().close()}
  >
    <svg viewBox="0 0 10 10" aria-hidden="true">
      <path
        d="M1.8 1.5 5 4.7 8.2 1.5 8.5 1.8 5.3 5 8.5 8.2 8.2 8.5 5 5.3 1.8 8.5 1.5 8.2 4.7 5 1.5 1.8Z"
        fill="currentColor"
      />
    </svg>
  </button>
</div>
