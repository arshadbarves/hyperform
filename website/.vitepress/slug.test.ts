import { fileURLToPath } from "node:url";
import { createMarkdownRenderer } from "vitepress";
import { describe, expect, it } from "vitest";
import { githubSlug } from "./slug";

describe("githubSlug", () => {
  it("keeps the hyphen left by dropped trailing punctuation", () => {
    expect(githubSlug("Save a note with #")).toBe("save-a-note-with-");
  });

  it("keeps a leading digit", () => {
    expect(githubSlug("3. What to update for each kind of change")).toBe("3-what-to-update-for-each-kind-of-change");
  });

  it("gives the ids the home page links to", () => {
    expect(githubSlug("Steer, interrupt, cancel")).toBe("steer-interrupt-cancel");
    expect(githubSlug("Checkpoints and rewind")).toBe("checkpoints-and-rewind");
    expect(githubSlug("The sandbox")).toBe("the-sandbox");
  });

  it("drops punctuation but not the spaces around it", () => {
    expect(githubSlug("Start with /doctor")).toBe("start-with-doctor");
    expect(githubSlug("Where do I change X?")).toBe("where-do-i-change-x");
    expect(githubSlug("Command (protocol)")).toBe("command-protocol");
    expect(githubSlug("4.3 Bits UI — use the kit, not the package")).toBe("43-bits-ui--use-the-kit-not-the-package");
  });

  it("keeps underscores, hyphens, accents and other scripts", () => {
    expect(githubSlug("default_config.toml")).toBe("default_configtoml");
    expect(githubSlug("z-engine-tools")).toBe("z-engine-tools");
    expect(githubSlug("Café Übersicht 日本語")).toBe("café-übersicht-日本語");
  });

  it("does not trim", () => {
    expect(githubSlug(" Padded ")).toBe("-padded-");
  });
});

describe("markdown.anchor.slugify", () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));

  it("ids headings with inline code by their text, as GitHub does", async () => {
    const md = await createMarkdownRenderer(root, { anchor: { slugify: githubSlug } });
    const html = md.render(
      "## Save a note with `#`\n\n## GUI shell (`crates/z-engine-gui/src-tauri/src`)\n\n## Save a note with `#`\n",
    );

    expect(html).toContain(`<h2 id="save-a-note-with-" tabindex="-1">`);
    expect(html).toContain(`href="#save-a-note-with-"`);
    expect(html).toContain(`<h2 id="gui-shell-cratesz-engine-guisrc-taurisrc" tabindex="-1">`);
    expect(html).toContain(`<h2 id="save-a-note-with--1" tabindex="-1">`);
  });
});
