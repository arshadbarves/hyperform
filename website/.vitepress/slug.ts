const DROPPED = /[^\p{L}\p{M}\p{N}\p{Pc}\- ]/gu;

/**
 * The id GitHub gives a heading with this text, so anchors written for GitHub
 * work on the site. Repeated headings get their `-1`, `-2` suffix from
 * markdown-it-anchor, as on GitHub.
 */
export function githubSlug(text: string): string {
  return text.toLowerCase().replace(DROPPED, "").replaceAll(" ", "-");
}
