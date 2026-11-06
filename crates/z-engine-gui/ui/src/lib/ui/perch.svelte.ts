import { untrack } from "svelte";
import type { PerchId, PerchKind, PerchRect } from "../domain/pet/perches";

interface Entry {
  node: HTMLElement;
  kind: PerchKind;
}

/**
 * The places on screen the pet can be, registered by the `perch` action.
 * `version` changes whenever one appears or leaves. Their rects are measured
 * on the next frame after a perch appears, leaves or resizes, or `measure`
 * is called: by then the DOM has settled, so reading layout forces no extra
 * layout in the middle of an update.
 */
class PerchRegistry {
  version = $state(0);
  #rects = $state.raw<PerchRect[]>([]);
  #entries = new Map<PerchId, Entry>();
  #observer: ResizeObserver | null = null;
  #frame = 0;

  add(id: PerchId, node: HTMLElement, kind: PerchKind) {
    this.#entries.set(id, { node, kind });
    if (typeof ResizeObserver === "function") {
      this.#observer ??= new ResizeObserver(() => this.measure());
      this.#observer.observe(node);
    }
    this.#changed();
  }

  remove(id: PerchId, node: HTMLElement) {
    if (this.#entries.get(id)?.node !== node) return;
    this.#entries.delete(id);
    this.#observer?.unobserve(node);
    this.#changed();
  }

  #changed() {
    this.version = untrack(() => this.version) + 1;
    this.measure();
  }

  /** Measures every perch again on the next frame (once, however often it is asked). */
  measure() {
    if (this.#frame) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = 0;
      const next = [...this.#entries].map(([id, entry]) => {
        const box = entry.node.getBoundingClientRect();
        return { id, kind: entry.kind, left: box.left, top: box.top, width: box.width, height: box.height };
      });
      if (!sameRects(this.#rects, next)) this.#rects = next;
    });
  }

  node(id: PerchId): HTMLElement | null {
    void this.version;
    return this.#entries.get(id)?.node ?? null;
  }

  /** Where each perch was at the last measure, in viewport pixels. */
  rects(): PerchRect[] {
    return this.#rects;
  }
}

function sameRects(a: PerchRect[], b: PerchRect[]): boolean {
  return (
    a.length === b.length &&
    a.every((r, i) => {
      const s = b[i];
      return r.id === s.id && r.kind === s.kind && r.left === s.left && r.top === s.top && r.width === s.width && r.height === s.height;
    })
  );
}

export const perches = new PerchRegistry();

type PerchParams = { id: PerchId; kind: PerchKind };

/** Marks an element as a place the pet can be: a slot it sits in, or an edge it stands on. */
export function perch(node: HTMLElement, params: PerchParams) {
  let current = params;
  perches.add(current.id, node, current.kind);
  return {
    update(next: PerchParams) {
      perches.remove(current.id, node);
      current = next;
      perches.add(current.id, node, current.kind);
    },
    destroy() {
      perches.remove(current.id, node);
    },
  };
}
