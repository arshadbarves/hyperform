import { fileURLToPath } from "node:url";
import { mergeAlias, type Plugin, type ResolverFunction } from "vite";

const BARE = /^(?![a-zA-Z]:)[\w@](?!.*:\/\/)/;
const SPECIAL = /[.*+?^${}()|[\]\\]/g;

/** Matches exactly the ids Vite resolves for `optimizeDeps.include`: `a > b` resolves `b`. */
export function includedIds(include: readonly string[]): RegExp | undefined {
  const ids = include.map((entry) => entry.slice(entry.lastIndexOf(">") + 1).trim().replace(SPECIAL, "\\$&"));
  return ids.length ? new RegExp(`^(?:${ids.join("|")})$`) : undefined;
}

/**
 * Pages live outside website/, where no node_modules can be found by walking
 * up, so their bare imports (vue, vue/server-renderer) resolve from here. So
 * do the dev optimizer's `optimizeDeps.include` entries, which Vite resolves
 * from its root with aliases and node resolution only, never plugin hooks.
 */
export function packagesFromWebsite(): Plugin {
  const website = fileURLToPath(new URL("../", import.meta.url));
  const importer = `${website}index.md`;
  const fromWebsite: ResolverFunction = function (source, _from, options) {
    return this.resolve(source, importer, { ...options, skipSelf: true });
  };
  return {
    name: "z-engine:packages-from-website",
    enforce: "pre",
    config: {
      // Post, to see the entries VitePress and withMermaid add. Appended last,
      // so an id that already has an alias (VitePress's deduped vue) keeps it.
      order: "post",
      handler(config, { command }) {
        const find = includedIds(config.optimizeDeps?.include ?? []);
        if (command !== "serve" || !find) return;
        const alias = { find, replacement: "$&", customResolver: fromWebsite };
        config.resolve = { ...config.resolve, alias: mergeAlias([alias], config.resolve?.alias) };
      },
    },
    resolveId(source, from, options) {
      if (!from || from.startsWith(website) || !BARE.test(source)) return null;
      return fromWebsite.call(this, source, from, options);
    },
  };
}
