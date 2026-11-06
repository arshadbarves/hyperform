import { statSync } from "node:fs";
import { join, posix } from "node:path";
import type { MarkdownRenderer } from "vitepress";

export const REPO_URL = "https://github.com/arshadbarves/z-engine";
export const BRANCH = "release";

const ROOT_PAGES = new Set(["README.md", "AGENTS.md", "CHANGELOG.md"]);
const NOT_RELATIVE = /^(?:[a-z][a-z\d+.-]*:|\/|#)/i;

/** Whether a repo-relative path is one of the markdown files the site publishes. */
export function isPage(path: string): boolean {
  return ROOT_PAGES.has(path) || (path.startsWith("docs/") && path.endsWith(".md"));
}

/**
 * Resolves a relative `href` on the page at `pagePath` (both repo-relative).
 * A target that is not a published page becomes its GitHub URL (`tree/` for
 * folders); page links, site links, anchors and external links come back
 * unchanged.
 */
export function githubLink(pagePath: string, href: string, isFolder: (path: string) => boolean = () => false): string {
  if (NOT_RELATIVE.test(href)) return href;
  const cut = href.search(/[?#]/);
  const target = cut < 0 ? href : href.slice(0, cut);
  const suffix = cut < 0 ? "" : href.slice(cut);
  if (target === "") return href;

  const resolved = posix.join(posix.dirname(pagePath), target);
  if (resolved === ".." || resolved.startsWith("../") || isPage(resolved)) return href;

  const path = resolved.replace(/\/$/, "").replace(/^\.$/, "");
  const kind = target.endsWith("/") || path === "" || isFolder(path) ? "tree" : "blob";
  return `${REPO_URL}/${kind}/${BRANCH}${path && `/${path}`}${suffix}`;
}

/** markdown-it plugin: rewrites every link on a page with `githubLink`, checking folders under `root`. */
export function githubLinks(md: MarkdownRenderer, root: string): void {
  const isFolder = (path: string) => statSync(join(root, path), { throwIfNoEntry: false })?.isDirectory() ?? false;
  md.core.ruler.push("github_links", (state) => {
    const page: unknown = state.env?.relativePath;
    if (typeof page !== "string") return;
    for (const token of state.tokens) {
      for (const child of token.children ?? []) {
        const href = child.type === "link_open" ? child.attrGet("href") : null;
        if (href !== null) child.attrSet("href", githubLink(page, href, isFolder));
      }
    }
  });
}
