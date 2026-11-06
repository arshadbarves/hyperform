import { fileURLToPath } from "node:url";
import { defineConfig } from "vitepress";
import { withMermaid } from "vitepress-plugin-mermaid";
import { packagesFromWebsite } from "./imports";
import { BRANCH, REPO_URL, githubLinks } from "./links";
import { sidebar } from "./sidebar";
import { githubSlug } from "./slug";

const root = fileURLToPath(new URL("../../", import.meta.url));
const base = "/hyperform/";

export default withMermaid(
  defineConfig({
    title: "HyperForm AI",
    description: "Local-first, intelligent AI development environment.",
    base,
    srcDir: "..",
    srcExclude: [
      "crates/**",
      "target/**",
      "scripts/**",
      "tmp/**",
      ".claude/**",
      ".github/**",
      "website/.vitepress/**",
      "**/node_modules/**",
    ],
    rewrites: { "website/index.md": "index.md" },
    cleanUrls: true,
    lastUpdated: true,
    appearance: "dark",
    head: [
      ["link", { rel: "icon", type: "image/svg+xml", href: `${base}favicon.svg` }],
      ["meta", { property: "og:image", content: "https://arshadbarves.github.io/hyperform/og.png" }],
      ["meta", { property: "og:image:alt", content: "HyperForm AI" }],
      ["meta", { name: "twitter:card", content: "summary" }],
      ["meta", { name: "twitter:image", content: "https://arshadbarves.github.io/hyperform/og.png" }],
    ],
    markdown: { anchor: { slugify: githubSlug }, config: (md) => githubLinks(md, root) },
    mermaid: { theme: "neutral" },
    vite: {
      plugins: [packagesFromWebsite()],
      // Imported only from inside node_modules (the plugin's Mermaid.vue), so
      // never discovered; pre-bundling converts CommonJS deps such as fastdom
      // that withMermaid's own include list misses.
      optimizeDeps: { include: ["mermaid"] },
      publicDir: fileURLToPath(new URL("../public", import.meta.url)),
      server: { watch: { ignored: ["**/target/**", "**/crates/**", "**/tmp/**", "**/.worktrees/**"] } },
    },
    themeConfig: {
      logo: "/logo.svg",
      nav: [
        { text: "Guide", link: "/docs/user-guide/README", activeMatch: "^/docs/user-guide/" },
        { text: "How it works", link: "/docs/how-it-works/README", activeMatch: "^/docs/how-it-works/" },
        {
          text: "Contributing",
          link: "/AGENTS",
          activeMatch: "^/(AGENTS|docs/(AGENTS|status|architecture/|engineering/|design/))",
        },
        { text: "Changelog", link: "/CHANGELOG" },
        {
          text: "GitHub",
          items: [
            { text: "Repository", link: REPO_URL },
            { text: "Releases", link: `${REPO_URL}/releases` },
          ],
        },
      ],
      sidebar: sidebar(root),
      socialLinks: [{ icon: "github", link: REPO_URL }],
      search: { provider: "local" },
      editLink: { pattern: `${REPO_URL}/edit/${BRANCH}/:path`, text: "Edit this page on GitHub" },
    },
  }),
);
