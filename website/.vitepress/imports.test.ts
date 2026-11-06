import { fileURLToPath } from "node:url";
import { resolveConfig } from "vite";
import { describe, expect, it } from "vitest";
import { includedIds, packagesFromWebsite } from "./imports";

describe("includedIds", () => {
  it("matches the id each entry resolves, and only that id", () => {
    const find = includedIds(["dayjs", "vitepress > @vueuse/core", "vitepress > mark.js/src/vanilla.js"]);
    for (const id of ["dayjs", "@vueuse/core", "mark.js/src/vanilla.js"]) expect(find?.test(id)).toBe(true);
    for (const id of ["dayjs/plugin/isoWeek.js", "vitepress", "@vueuse/core-x", "markXjs/src/vanilla.js"]) {
      expect(find?.test(id)).toBe(false);
    }
  });

  it("is undefined without entries", () => {
    expect(includedIds([])).toBeUndefined();
  });
});

describe("packagesFromWebsite", () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const website = fileURLToPath(new URL("../", import.meta.url));
  const vue = `${website}node_modules/vue/dist/vue.runtime.esm-bundler.js`;

  it("lets the dev optimizer resolve optimizeDeps.include from the repo root", async () => {
    const config = await resolveConfig(
      {
        root,
        configFile: false,
        logLevel: "silent",
        plugins: [packagesFromWebsite()],
        // As VitePress and its Vue plugin set it up: deduped, vue resolves only through its alias.
        resolve: { alias: [{ find: /^vue$/, replacement: vue }], dedupe: ["vue"] },
        optimizeDeps: { include: ["vue", "dayjs", "vitepress > @vueuse/core"] },
      },
      "serve",
    );
    const resolve = config.createResolver({ asSrc: false, scan: true });
    expect(await resolve("vue")).toBe(vue);
    expect(await resolve("dayjs")).toContain(`${website}node_modules/dayjs/`);
    expect(await resolve("@vueuse/core")).toContain(`${website}node_modules/@vueuse/core/`);
  });
});
