import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { pageTitle, sidebar, unlisted } from "./sidebar";

describe("pageTitle", () => {
  it("reads the first level-one heading", () => {
    expect(pageTitle("Intro line\n\n# Getting started\n\n## Install\n")).toBe("Getting started");
    expect(pageTitle("# Hooks ##\n")).toBe("Hooks");
  });

  it("skips headings inside code fences", () => {
    expect(pageTitle("```toml\n# a comment\n```\n~~~md\n# Example\n~~~\n# Real title\n")).toBe("Real title");
  });

  it("escapes HTML and drops code marks", () => {
    expect(pageTitle("# Style & `Coding` <Guide>\n")).toBe("Style &amp; Coding &lt;Guide&gt;");
  });

  it("returns undefined without a heading", () => {
    expect(pageTitle("## Only a section\n")).toBeUndefined();
  });
});

describe("unlisted", () => {
  it("lists the pages no section links to", () => {
    const sections = [{ text: "Guide", pages: ["docs/a.md", "docs/b.md"] }];
    expect(unlisted(["docs/a.md", "docs/c.md", "docs/b.md", "docs/d.md"], sections)).toEqual(["docs/c.md", "docs/d.md"]);
  });
});

describe("sidebar", () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));

  it("covers every docs page, titled from its heading", () => {
    const groups = sidebar(root);
    const items = groups.flatMap((group) => group.items ?? []);
    expect(groups.map((group) => group.text)).toEqual(["Overview", "User guide", "How it works", "Contributing"]);
    expect(items).toContainEqual({ text: "Getting started", link: "/docs/user-guide/01-getting-started" });
    expect(items).toContainEqual({ text: "Glossary", link: "/docs/how-it-works/glossary" });
  });
});
