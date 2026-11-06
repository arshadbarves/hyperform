export interface ScrollControllerOptions {
  /** Distance from bottom in pixels to consider "at bottom". Defaults to 50. */
  bottomThreshold?: number;
}

export function createScrollController(options: ScrollControllerOptions = {}) {
  const threshold = options.bottomThreshold ?? 50;

  let containerEl = $state<HTMLDivElement | undefined>(undefined);
  let isPinned = $state(true);
  let showJump = $state(false);

  let lastScrollTop = 0;
  let isProgrammatic = false;
  let scrollRaf = 0;
  let lastUserMsgId: string | null = null;
  let lastSessionId: string | null = null;
  let resizeObserver: ResizeObserver | null = null;

  function scrollToBottom(smooth = false) {
    if (!containerEl) return;
    cancelAnimationFrame(scrollRaf);
    const el = containerEl;
    scrollRaf = requestAnimationFrame(() => {
      isProgrammatic = true;
      if (smooth) {
        el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
      } else {
        el.scrollTop = el.scrollHeight;
      }
      lastScrollTop = el.scrollTop;
      requestAnimationFrame(() => {
        isProgrammatic = false;
      });
    });
  }

  function handleScroll() {
    if (!containerEl) return;
    const el = containerEl;
    const currentScrollTop = el.scrollTop;
    const gap = el.scrollHeight - currentScrollTop - el.clientHeight;

    if (isProgrammatic) {
      lastScrollTop = currentScrollTop;
      return;
    }

    if (gap <= threshold) {
      // User scrolled all the way to the bottom
      isPinned = true;
      showJump = false;
    } else if (currentScrollTop < lastScrollTop - 1) {
      // User scrolled UP
      isPinned = false;
      showJump = true;
    } else {
      // Scrolled down but not at bottom yet
      isPinned = false;
      showJump = true;
    }

    lastScrollTop = currentScrollTop;
  }

  function handleWheel(e: WheelEvent) {
    if (e.deltaY < 0) {
      isPinned = false;
      showJump = true;
    }
  }

  function handlePointerDown() {
    if (containerEl) {
      lastScrollTop = containerEl.scrollTop;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "PageUp" || e.key === "Home" || (e.key === "ArrowUp" && !e.ctrlKey && !e.metaKey)) {
      isPinned = false;
      showJump = true;
    } else if (e.key === "End") {
      isPinned = true;
      showJump = false;
      scrollToBottom(true);
    }
  }

  function bindContainer(node: HTMLDivElement | undefined) {
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }

    containerEl = node;

    if (!node) return;

    lastScrollTop = node.scrollTop;

    node.addEventListener("scroll", handleScroll, { passive: true });
    node.addEventListener("wheel", handleWheel, { passive: true });
    node.addEventListener("pointerdown", handlePointerDown, { passive: true });
    node.addEventListener("keydown", handleKeydown, { passive: true });

    if (typeof ResizeObserver !== "undefined") {
      resizeObserver = new ResizeObserver(() => {
        if (isPinned) {
          scrollToBottom(false);
        } else {
          const gap = node.scrollHeight - node.scrollTop - node.clientHeight;
          showJump = gap > threshold;
        }
      });
      resizeObserver.observe(node);
    }

    return () => {
      cancelAnimationFrame(scrollRaf);
      node.removeEventListener("scroll", handleScroll);
      node.removeEventListener("wheel", handleWheel);
      node.removeEventListener("pointerdown", handlePointerDown);
      node.removeEventListener("keydown", handleKeydown);
      if (resizeObserver) {
        resizeObserver.disconnect();
        resizeObserver = null;
      }
    };
  }

  /** Call on every transcript change; `lastUserId` is the newest prompt's message id. */
  function onContentUpdated(sessionId: string | null, lastUserId: string | null) {
    if (sessionId !== lastSessionId) {
      lastSessionId = sessionId;
      lastUserMsgId = lastUserId;
      isPinned = true;
      showJump = false;
      scrollToBottom(false);
      return;
    }

    // A new prompt always re-pins the view to the bottom.
    if (lastUserId !== null && lastUserId !== lastUserMsgId) {
      lastUserMsgId = lastUserId;
      isPinned = true;
      showJump = false;
      scrollToBottom(false);
      return;
    }

    // While streaming / working / receiving tool outputs
    if (isPinned && containerEl) {
      scrollToBottom(false);
    }
  }

  function jumpToLatest() {
    isPinned = true;
    showJump = false;
    scrollToBottom(true);
  }

  function jumpToElement(elementId: string) {
    const target = document.getElementById(elementId);
    if (target && containerEl) {
      isPinned = false;
      showJump = true;
      cancelAnimationFrame(scrollRaf);
      const cRect = containerEl.getBoundingClientRect();
      const tRect = target.getBoundingClientRect();
      const offset = tRect.top - cRect.top + containerEl.scrollTop - 20;

      isProgrammatic = true;
      containerEl.scrollTo({ top: Math.max(0, offset), behavior: "smooth" });
      lastScrollTop = Math.max(0, offset);
      setTimeout(() => {
        isProgrammatic = false;
      }, 500);
    }
  }

  return {
    get isPinned() {
      return isPinned;
    },
    get showJump() {
      return showJump;
    },
    get containerEl() {
      return containerEl;
    },
    bindContainer,
    handleScroll,
    handleWheel,
    handlePointerDown,
    handleKeydown,
    onContentUpdated,
    jumpToLatest,
    jumpToElement,
    scrollToBottom,
  };
}
