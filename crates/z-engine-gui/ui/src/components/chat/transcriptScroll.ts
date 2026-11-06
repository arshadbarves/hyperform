type Seen = { height: number; pending: boolean };

/**
 * Keeps what you are reading still while turns above it change height: a
 * placeholder turn that fills in as it nears the screen, or a turn skipped by
 * `content-visibility` that paints again. The scroller turns CSS scroll
 * anchoring off (WebKit has none), so this is the only anchor. A placeholder
 * that fills in while you sit at the bottom keeps you at the bottom; a turn
 * you unfold yourself is left alone.
 */
export function anchorTurns(list: HTMLElement, scroller: HTMLElement): () => void {
  const seen = new WeakMap<Element, Seen>();
  let following = isAtBottom(scroller);

  const resize = new ResizeObserver((entries) => {
    const top = scroller.getBoundingClientRect().top;
    let shift = 0;
    let pin = false;
    for (const entry of entries) {
      const el = entry.target;
      const height = entry.borderBoxSize[0]?.blockSize ?? el.getBoundingClientRect().height;
      const before = seen.get(el);
      seen.set(el, { height, pending: el.classList.contains("is-pending") });
      if (!before || before.height === height || !el.isConnected) continue;
      if (before.pending && following) pin = true;
      else if (el.getBoundingClientRect().top + before.height <= top + 1) shift += height - before.height;
    }
    if (pin) scroller.scrollTop = scroller.scrollHeight;
    else if (shift) scroller.scrollTop += shift;
  });
  const watch = (nodes: Iterable<Node>) => {
    for (const node of nodes) if (node instanceof HTMLElement && node.classList.contains("turn")) resize.observe(node);
  };
  const mutations = new MutationObserver((records) => {
    for (const record of records) {
      watch(record.addedNodes);
      for (const node of record.removedNodes) if (node instanceof Element) resize.unobserve(node);
    }
  });
  const onScroll = () => {
    following = isAtBottom(scroller);
  };

  watch(list.children);
  mutations.observe(list, { childList: true });
  scroller.addEventListener("scroll", onScroll, { passive: true });
  return () => {
    scroller.removeEventListener("scroll", onScroll);
    mutations.disconnect();
    resize.disconnect();
  };
}

/** Calls `onReach` whenever `marker` comes within a couple of screens of the scroller's top edge. */
export function observeTop(marker: Element, scroller: HTMLElement, onReach: () => void): () => void {
  const observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) onReach();
    },
    { root: scroller, rootMargin: "1600px 0px 0px 0px" },
  );
  observer.observe(marker);
  return () => observer.disconnect();
}

export function isAtBottom(scroller: HTMLElement, slack = 32): boolean {
  return scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight <= slack;
}
