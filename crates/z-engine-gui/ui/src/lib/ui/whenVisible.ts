type Waiter = () => void;

/** How far outside the visible area an element counts as reached. */
const MARGIN = "1000px 0px";

const waiters = new Map<Element, Waiter>();
const byRoot = new WeakMap<Element, IntersectionObserver>();
let page: IntersectionObserver | null = null;

/** The nearest ancestor that scrolls vertically, or null for the page itself. */
export function scrollParent(el: Element): HTMLElement | null {
  for (let node = el.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === "auto" || overflow === "scroll") return node;
  }
  return null;
}

// The scroller each ancestor sits in (itself, if it scrolls), noted as
// lookups walk up. A long transcript mounting at once then reads each
// ancestor's style once, not once per turn and code block. Kept only until
// this task's microtasks run, so it never goes stale.
const within = new Map<Element, HTMLElement | null>();

function scrollerOf(el: Element): HTMLElement | null {
  if (within.size === 0) queueMicrotask(() => within.clear());
  const path: Element[] = [];
  let found: HTMLElement | null = null;
  for (let node = el.parentElement; node; node = node.parentElement) {
    if (within.has(node)) {
      found = within.get(node) ?? null;
      break;
    }
    path.push(node);
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === "auto" || overflow === "scroll") {
      found = node;
      break;
    }
  }
  for (const node of path) within.set(node, found);
  return found;
}

function reached(entries: IntersectionObserverEntry[], observer: IntersectionObserver) {
  for (const entry of entries) {
    if (!entry.isIntersecting) continue;
    observer.unobserve(entry.target);
    const run = waiters.get(entry.target);
    waiters.delete(entry.target);
    run?.();
  }
}

// A scroller clips what it holds, so the margin only reaches ahead when the
// scroller itself is the observer's root.
function observerFor(root: HTMLElement | null): IntersectionObserver {
  if (!root) return (page ??= new IntersectionObserver(reached, { rootMargin: MARGIN }));
  let observer = byRoot.get(root);
  if (!observer) {
    observer = new IntersectionObserver(reached, { root, rootMargin: MARGIN });
    byRoot.set(root, observer);
  }
  return observer;
}

/**
 * Runs `run` once, the first time `el` comes within 1000px of the visible
 * part of its scroller. Returns a cancel function.
 */
export function whenVisible(el: Element, run: Waiter): () => void {
  if (typeof IntersectionObserver === "undefined") {
    run();
    return () => {};
  }
  const observer = observerFor(scrollerOf(el));
  waiters.set(el, run);
  observer.observe(el);
  return () => {
    if (waiters.delete(el)) observer.unobserve(el);
  };
}
