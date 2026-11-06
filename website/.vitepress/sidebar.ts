import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import type { DefaultTheme } from "vitepress";

type Section = { text: string; pages: string[] };

/** Every docs page must appear here; the build fails otherwise. */
const SECTIONS: Section[] = [
  { text: "Overview", pages: ["README.md", "docs/README.md", "CHANGELOG.md"] },
  {
    text: "User guide",
    pages: [
      "docs/user-guide/README.md",
      "docs/user-guide/01-getting-started.md",
      "docs/user-guide/02-everyday-use.md",
      "docs/user-guide/03-permissions-and-safety.md",
      "docs/user-guide/04-agents.md",
      "docs/user-guide/05-plan-mode-questions-and-todos.md",
      "docs/user-guide/06-commands-and-skills.md",
      "docs/user-guide/07-memory-and-context.md",
      "docs/user-guide/08-hooks.md",
      "docs/user-guide/09-mcp-and-code-intelligence.md",
      "docs/user-guide/10-verification.md",
      "docs/user-guide/11-models-providers-and-cost.md",
      "docs/user-guide/12-settings-reference.md",
      "docs/user-guide/13-sessions-and-data.md",
      "docs/user-guide/14-troubleshooting.md",
      "docs/user-guide/15-experimental-features.md",
      "docs/user-guide/16-decision-features.md",
      "docs/user-guide/17-decision-settings.md",
      "docs/user-guide/18-work-plans.md",
      "docs/user-guide/19-pausing-and-continuing-plans.md",
    ],
  },
  {
    text: "How it works",
    pages: [
      "docs/how-it-works/README.md",
      "docs/how-it-works/features-core.md",
      "docs/how-it-works/features-interaction.md",
      "docs/how-it-works/features-agents-and-context.md",
      "docs/how-it-works/features-integrations-and-safety.md",
      "docs/how-it-works/features-desktop-app.md",
      "docs/how-it-works/features-desktop-screens.md",
      "docs/how-it-works/features-desktop-pet.md",
      "docs/how-it-works/features-experimental-and-decisions.md",
      "docs/how-it-works/features-decision-uses.md",
      "docs/how-it-works/features-work-plans.md",
      "docs/how-it-works/features-work-parallel.md",
      "docs/how-it-works/features-work-resume.md",
      "docs/how-it-works/crates.md",
      "docs/how-it-works/crates-engine-and-app.md",
      "docs/how-it-works/where-to-change.md",
      "docs/how-it-works/glossary.md",
      "docs/how-it-works/glossary-desktop.md",
      "docs/how-it-works/glossary-experimental.md",
    ],
  },
  {
    text: "Contributing",
    pages: [
      "AGENTS.md",
      "docs/AGENTS.md",
      "docs/architecture/v2-engine.md",
      "docs/architecture/v2-engine-services.md",
      "docs/architecture/v2-engine-work.md",
      "docs/engineering/style-guide.md",
      "docs/design/gui-ui-guide.md",
      "docs/design/gui-surfaces.md",
      "docs/status.md",
    ],
  },
];

const FENCE = /^\s*(```|~~~)/;
const HEADING = /^#\s+(.+?)(?:\s+#+)?\s*$/;
const ENTITIES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;" };

/** The first `#` heading outside code fences, as sidebar HTML. */
export function pageTitle(markdown: string): string | undefined {
  let fenced = false;
  for (const line of markdown.split(/\r?\n/)) {
    if (FENCE.test(line)) fenced = !fenced;
    const heading = fenced ? null : HEADING.exec(line);
    if (heading) return heading[1].replaceAll("`", "").replace(/[&<>]/g, (c) => ENTITIES[c]);
  }
  return undefined;
}

/** The pages that no section lists, in their given order. */
export function unlisted(pages: string[], sections: Section[] = SECTIONS): string[] {
  const listed = new Set(sections.flatMap((section) => section.pages));
  return pages.filter((page) => !listed.has(page));
}

/** The sidebar for the repo at `root`; throws when a docs page is missing from it. */
export function sidebar(root: string): DefaultTheme.SidebarItem[] {
  const docs = readdirSync(join(root, "docs"), { recursive: true, encoding: "utf8" })
    .filter((name) => name.endsWith(".md"))
    .map((name) => `docs/${name.replaceAll("\\", "/")}`)
    .sort();
  const missing = unlisted(docs);
  if (missing.length > 0) {
    const list = missing.map((page) => `  ${page}`).join("\n");
    throw new Error(`These docs pages are not in the website sidebar; add them to website/.vitepress/sidebar.ts:\n${list}`);
  }
  return SECTIONS.map(({ text, pages }) => ({
    text,
    collapsed: false,
    items: pages.map((page) => ({ text: title(root, page), link: `/${page.replace(/\.md$/, "")}` })),
  }));
}

function title(root: string, page: string): string {
  const text = pageTitle(readFileSync(join(root, page), "utf8"));
  if (text === undefined) throw new Error(`${page} needs a "# " heading to title its sidebar entry`);
  return text;
}
