<script lang="ts">
  import { lastPromptId } from "$lib/domain/timeline/blocks";
  import { sessions } from "$lib/runtime";
  import { currentStage } from "$lib/stores/stage.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { appear, prefersReducedMotion, springEasingCss } from "$lib/ui/motion";
  import { createScrollController } from "$lib/ui/scrollController.svelte";
  import { springs } from "$lib/ui/springs";
  import { workspaceStore } from "$lib/workspaces";
  import Composer from "../chat/Composer.svelte";
  import Transcript from "../chat/Transcript.svelte";
  import HomeCards from "../home/HomeCards.svelte";
  import HomeHeader from "../home/HomeHeader.svelte";
  import InboxView from "../inbox/InboxView.svelte";
  import JumpLatest from "./JumpLatest.svelte";

  /**
   * The content sheet: the project home, the chat or the inbox. Each stage
   * fades up as it replaces the last (transform and opacity only), and
   * scrolls under the transparent title zone. The composer is one instance
   * throughout: centred on the home screen, it glides down when the first
   * message turns home into a chat, and there floats over the bottom of the
   * sheet with the transcript scrolling under it.
   */
  const STAGE_IN = { y: 8, scale: 1 };
  const scroller = createScrollController({ bottomThreshold: 24 });
  const workspaces = bindStore(workspaceStore);
  let transcriptEl: HTMLDivElement | undefined = $state();
  let dock: HTMLDivElement | undefined = $state();
  let dockHeight = $state(0);
  let dockTop: number | null = null;

  const view = $derived(sessions.active);
  const lastPrompt = $derived(lastPromptId(view?.messages));
  const stage = $derived(currentStage());
  const root = $derived(view?.info?.projectRoot ?? workspaces.current.active);
  const composerShown = $derived(stage === "chat" || (stage === "home" && root !== null));

  $effect(() => scroller.bindContainer(transcriptEl));

  $effect(() => {
    userSignals.scrolledBack = scroller.showJump;
  });

  $effect(() => {
    void view;
    scroller.onContentUpdated(sessions.activeId, lastPrompt);
  });

  $effect.pre(() => {
    void stage;
    dockTop = dock ? dock.getBoundingClientRect().top : null;
  });

  $effect(() => {
    void stage;
    const before = dockTop;
    dockTop = null;
    if (!dock || before === null || prefersReducedMotion()) return;
    const shift = before - dock.getBoundingClientRect().top;
    if (Math.abs(shift) < 4) return;
    dock.animate([{ transform: `translateY(${shift}px)` }, { transform: "none" }], {
      duration: springs.smooth.duration,
      easing: springEasingCss("smooth"),
    });
  });
</script>

<div class="canvas-pane" data-stage={stage} style:--dock-h={stage === "chat" && composerShown ? `${dockHeight}px` : null}>
  {#if stage === "chat"}
    <div class="transcript-wrap" in:appear={STAGE_IN}>
      {#if sessions.hydrating}
        <div class="hydrate-shimmer" aria-label="Restoring chat"></div>
      {/if}
      <div class="transcript stage-scroll" bind:this={transcriptEl}>
        <Transcript />
      </div>
    </div>
  {:else if stage === "home"}
    <div class="home-top" in:appear={STAGE_IN}>
      <HomeHeader {root} />
    </div>
  {:else}
    <div class="inbox-wrap stage-scroll" in:appear={STAGE_IN}>
      <InboxView />
    </div>
  {/if}

  {#if stage !== "home"}
    <div class="edge-blur-top" aria-hidden="true"></div>
  {/if}
  {#if stage === "chat" && composerShown}
    <div class="edge-blur-bottom" aria-hidden="true"></div>
  {/if}
  {#if stage === "chat" && scroller.showJump}
    <div class="jump-slot">
      <JumpLatest onJump={() => scroller.jumpToLatest()} />
    </div>
  {/if}

  {#if composerShown}
    <div class="composer-dock" class:is-hero={stage === "home"} bind:this={dock} bind:clientHeight={dockHeight}>
      <Composer />
    </div>
  {/if}

  {#if stage === "home" && root}
    <div class="home-bottom" in:appear={STAGE_IN}>
      <HomeCards {root} />
    </div>
  {/if}
</div>
