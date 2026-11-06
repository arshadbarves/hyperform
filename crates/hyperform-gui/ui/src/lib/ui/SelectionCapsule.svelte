<script lang="ts">
  import { prefersReducedMotion, springEasingCss } from "./motion";
  import { springs } from "./springs";

  /**
   * One fill under whichever item in its container matches `selector`; it
   * slides from item to item as the selection moves, and fades when nothing
   * is selected. Place it first inside a positioned container whose items
   * are positioned too, so they paint above it. Its new box is committed at
   * once and the slide plays on transform only.
   */
  type Props = { selector: string; class?: string };
  let { selector, class: className = "" }: Props = $props();

  type Box = { x: number; y: number; w: number; h: number };
  let el: HTMLSpanElement | undefined = $state();
  let box = $state<Box>({ x: 0, y: 0, w: 0, h: 0 });
  let visible = $state(false);
  let placed = $state(false);

  function slide(from: Box, to: Box) {
    if (!el || prefersReducedMotion() || to.w <= 0 || to.h <= 0) return;
    if (from.x === to.x && from.y === to.y && from.w === to.w && from.h === to.h) return;
    const offset = `translate(${from.x - to.x}px, ${from.y - to.y}px) scale(${from.w / to.w}, ${from.h / to.h})`;
    el.animate([{ transform: offset }, { transform: "none" }], {
      duration: springs.snappy.duration,
      easing: springEasingCss("snappy"),
    });
  }

  $effect(() => {
    const host = el?.parentElement;
    if (!host) return;
    const place = (animate: boolean) => {
      const target = host.querySelector<HTMLElement>(selector);
      if (!target) {
        visible = false;
        return;
      }
      const outer = host.getBoundingClientRect();
      const inner = target.getBoundingClientRect();
      const next = { x: inner.left - outer.left, y: inner.top - outer.top + host.scrollTop, w: inner.width, h: inner.height };
      if (animate && visible && placed) slide(box, next);
      box = next;
      visible = true;
      requestAnimationFrame(() => (placed = true));
    };
    place(false);
    const mutations = new MutationObserver(() => place(true));
    mutations.observe(host, { subtree: true, childList: true, attributes: true, attributeFilter: ["class", "aria-current"] });
    const sizes = new ResizeObserver(() => place(false));
    sizes.observe(host);
    return () => {
      mutations.disconnect();
      sizes.disconnect();
    };
  });
</script>

<span
  bind:this={el}
  class={`selection-capsule ${className}`}
  class:is-placed={placed}
  style:translate={`${box.x}px ${box.y}px`}
  style:width={`${box.w}px`}
  style:height={`${box.h}px`}
  style:opacity={visible ? 1 : 0}
  aria-hidden="true"
></span>
