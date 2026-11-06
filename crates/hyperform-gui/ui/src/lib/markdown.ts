import type { HastNode } from "svelte-exmarkdown";
import { gfmPlugin } from "svelte-exmarkdown/gfm";
import { createParser } from "svelte-exmarkdown/utils";

const parse = createParser([gfmPlugin()]);
/** Parsed blocks kept across chats; the least recently used goes first. */
const CACHE_SIZE = 800;
const cache = new Map<string, HastNode>();

/**
 * Markdown to the tree `Markdown.svelte` renders. Finished text is parsed
 * once and cached; pass `keep = false` for text still streaming, which would
 * only crowd the cache.
 */
export function parseMarkdown(text: string, keep = true): HastNode {
  const hit = cache.get(text);
  if (hit) {
    cache.delete(text);
    cache.set(text, hit);
    return hit;
  }
  const tree = parse(text) as HastNode;
  if (keep) {
    cache.set(text, tree);
    if (cache.size > CACHE_SIZE) cache.delete(cache.keys().next().value as string);
  }
  return tree;
}
