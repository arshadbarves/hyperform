import { fileURLToPath } from "node:url";
import { createMarkdownRenderer } from "vitepress";
import { describe, expect, it } from "vitest";
import { githubLink, githubLinks } from "./links";

const BLOB = "https://github.com/arshadbarves/z-engine/blob/release";
const TREE = "https://github.com/arshadbarves/z-engine/tree/release";
const PAGE = "docs/how-it-works/README.md";

describe("githubLink", () => {
  it("leaves links to published pages alone", () => {
    expect(githubLink(PAGE, "glossary.md")).toBe("glossary.md");
    expect(githubLink(PAGE, "glossary.md#model")).toBe("glossary.md#model");
    expect(githubLink(PAGE, "../user-guide/README.md")).toBe("../user-guide/README.md");
    expect(githubLink(PAGE, "../../AGENTS.md")).toBe("../../AGENTS.md");
    expect(githubLink("AGENTS.md", "docs/AGENTS.md")).toBe("docs/AGENTS.md");
    expect(githubLink("docs/README.md", "../CHANGELOG.md")).toBe("../CHANGELOG.md");
  });

  it("sends source files to GitHub", () => {
    expect(githubLink(PAGE, "../../crates/z-engine-llm/src/client.rs")).toBe(
      `${BLOB}/crates/z-engine-llm/src/client.rs`,
    );
    expect(githubLink("AGENTS.md", "scripts/check_docs_links.py")).toBe(`${BLOB}/scripts/check_docs_links.py`);
    expect(githubLink("docs/how-it-works/features-desktop-app.md", "../../.github/workflows/release.yml")).toBe(
      `${BLOB}/.github/workflows/release.yml`,
    );
  });

  it("sends markdown outside the published pages to GitHub", () => {
    expect(githubLink("AGENTS.md", ".claude/agents/docs-maintainer.md")).toBe(
      `${BLOB}/.claude/agents/docs-maintainer.md`,
    );
    expect(githubLink(PAGE, "../../crates/z-engine-prompts/prompts/auxiliary/compact.md")).toBe(
      `${BLOB}/crates/z-engine-prompts/prompts/auxiliary/compact.md`,
    );
  });

  it("keeps anchors and queries on GitHub links", () => {
    expect(githubLink(PAGE, "../../crates/z-engine-tools/src/names.rs#L10")).toBe(
      `${BLOB}/crates/z-engine-tools/src/names.rs#L10`,
    );
    expect(githubLink(PAGE, "../../scripts/dev-gui.sh?plain=1#L2")).toBe(`${BLOB}/scripts/dev-gui.sh?plain=1#L2`);
  });

  it("links folders as trees", () => {
    expect(githubLink(PAGE, "../../crates/z-engine-llm/src/")).toBe(`${TREE}/crates/z-engine-llm/src`);
    expect(githubLink(PAGE, "../../crates/z-engine-llm/src/#readme")).toBe(`${TREE}/crates/z-engine-llm/src#readme`);
    expect(githubLink(PAGE, "../../")).toBe(TREE);
  });

  it("asks the folder check about links without a trailing slash", () => {
    const isFolder = (path: string) => path === "crates/z-engine-prompts/prompts";
    expect(githubLink(PAGE, "../../crates/z-engine-prompts/prompts", isFolder)).toBe(
      `${TREE}/crates/z-engine-prompts/prompts`,
    );
    expect(githubLink(PAGE, "../../crates/z-engine-llm/src/client.rs", isFolder)).toBe(
      `${BLOB}/crates/z-engine-llm/src/client.rs`,
    );
  });

  it("leaves external links, anchors and site links alone", () => {
    for (const href of [
      "https://models.dev",
      "http://example.com/a.rs",
      "mailto:someone@example.com",
      "tauri://localhost",
      "#hooks",
      "?tab=1",
      "/docs/user-guide/README",
      "//example.com/a.rs",
    ]) {
      expect(githubLink(PAGE, href)).toBe(href);
    }
  });

  it("leaves links that escape the repository alone", () => {
    expect(githubLink(PAGE, "../../../elsewhere/file.rs")).toBe("../../../elsewhere/file.rs");
  });
});

describe("githubLinks", () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));

  it("hands source links to VitePress as external links and keeps page links internal", async () => {
    const md = await createMarkdownRenderer(root, { config: (md) => githubLinks(md, root) });
    const env: { relativePath: string; cleanUrls: boolean; links?: string[] } = {
      relativePath: PAGE,
      cleanUrls: true,
    };
    const html = md.render("[a](../../crates/z-engine-llm/src/client.rs) [b](../../crates) [c](glossary.md#model)", env);

    expect(html).toContain(`href="${BLOB}/crates/z-engine-llm/src/client.rs" target="_blank" rel="noreferrer"`);
    expect(html).toContain(`href="${TREE}/crates" target="_blank"`);
    expect(html).toContain(`href="./glossary#model"`);
    expect(env.links).toEqual(["./glossary#model"]);
  });
});
