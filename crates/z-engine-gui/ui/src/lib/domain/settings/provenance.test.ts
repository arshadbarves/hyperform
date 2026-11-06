import { describe, expect, it } from "vitest";
import type { LayerInfo } from "../../protocol/config/LayerInfo";
import type { LayerScope } from "../../protocol/config/LayerScope";
import {
  idOverride,
  layerActive,
  layerError,
  namedOverride,
  rawAt,
  tableNames,
  unionItems,
  valueSource,
  type Provenance,
} from "./provenance";

function layer(scope: LayerScope, exists = true, error: string | null = null): LayerInfo {
  return { scope, path: null, exists, error, note: null };
}

const ALL = [layer("default"), layer("user"), layer("project"), layer("projectLocal"), layer("env", false)];

function provenance(overrides: Partial<Provenance> = {}): Provenance {
  return { layers: ALL, raws: {}, effective: {}, ...overrides };
}

describe("rawAt", () => {
  it("walks nested tables and reports missing keys as undefined", () => {
    const raw = { model: { main: "m", fast: null }, list: [1] };
    expect(rawAt(raw, ["model", "main"])).toBe("m");
    expect(rawAt(raw, ["model", "fast"])).toBeNull();
    expect(rawAt(raw, ["model", "review"])).toBeUndefined();
    expect(rawAt(raw, ["list", "0"])).toBeUndefined();
    expect(rawAt(undefined, ["model"])).toBeUndefined();
  });
});

describe("valueSource", () => {
  it("is the default when no file sets the key", () => {
    expect(valueSource(provenance(), ["model", "main"])).toBe("default");
  });

  it("is the highest layer that sets the key", () => {
    const raws = { user: { model: { main: "u" } }, project: { model: { main: "p" } } };
    expect(valueSource(provenance({ raws }), ["model", "main"])).toBe("project");
    const withLocal = { ...raws, local: { model: { main: "l" } } };
    expect(valueSource(provenance({ raws: withLocal }), ["model", "main"])).toBe("projectLocal");
  });

  it("ignores a layer the loader skipped or a file that does not exist", () => {
    const raws = { user: { model: { main: "u" } }, project: { model: { main: "p" } } };
    const skipped = [layer("default"), layer("user"), layer("project", true, "bad effort"), layer("projectLocal", false)];
    expect(valueSource(provenance({ raws, layers: skipped }), ["model", "main"])).toBe("user");
    const missing = [layer("default"), layer("user", false), layer("project", false)];
    expect(valueSource(provenance({ raws, layers: missing }), ["model", "main"])).toBe("default");
  });

  it("attributes env-overridable keys to the environment when the effective value differs", () => {
    const layers = [...ALL.slice(0, 4), layer("env")];
    const raws = { user: { model: { main: "u" } } };
    const env = provenance({ layers, raws, effective: { model: { main: "from-env" } } });
    expect(valueSource(env, ["model", "main"])).toBe("env");
    const same = provenance({ layers, raws, effective: { model: { main: "u" } } });
    expect(valueSource(same, ["model", "main"])).toBe("user");
  });

  it("compares normalized URLs and kind aliases before blaming the environment", () => {
    const layers = [...ALL.slice(0, 4), layer("env")];
    const raws = { user: { provider: { base_url: "http://x/v1/", kind: "openai" } } };
    const effective = { provider: { base_url: "http://x/v1", kind: "openai_chat" } };
    const p = provenance({ layers, raws, effective });
    expect(valueSource(p, ["provider", "base_url"])).toBe("user");
    expect(valueSource(p, ["provider", "kind"])).toBe("user");
    const defaults = provenance({ layers, effective: { shell: { path: null }, model: { main: "anthropic/claude-sonnet-4.5" } } });
    expect(valueSource(defaults, ["shell", "path"])).toBe("default");
    expect(valueSource(defaults, ["model", "main"])).toBe("default");
  });

  it("never attributes other keys to the environment", () => {
    const layers = [...ALL.slice(0, 4), layer("env")];
    const p = provenance({ layers, effective: { model: { effort: "high" } } });
    expect(valueSource(p, ["model", "effort"])).toBe("default");
  });
});

describe("unionItems", () => {
  it("lists each rule once, in merge order, with every layer that lists it", () => {
    const raws = {
      user: { permissions: { allow: ["Read", "Bash(ls)"] } },
      project: { permissions: { allow: ["Bash(ls)", "Edit(src/**)", 3] } },
      local: { permissions: { allow: ["Read"] } },
    };
    expect(unionItems(provenance({ raws }), ["permissions", "allow"])).toEqual([
      { value: "Read", scopes: ["user", "local"] },
      { value: "Bash(ls)", scopes: ["user", "project"] },
      { value: "Edit(src/**)", scopes: ["project"] },
    ]);
  });
});

describe("overrides of named and id entries", () => {
  const raws = {
    user: { mcp: { servers: { fs: {}, git: {} } }, verification: { checks: [{ id: "unit" }, { id: "lint" }] } },
    project: { mcp: { servers: { fs: {} } }, verification: { checks: [{ id: "unit" }] } },
    local: { mcp: { servers: { fs: {} } } },
  };

  it("finds the highest merged layer defining the same server name", () => {
    const p = provenance({ raws });
    expect(namedOverride(p, "user", ["mcp", "servers"], "fs")).toBe("local");
    expect(namedOverride(p, "user", ["mcp", "servers"], "git")).toBeNull();
    expect(namedOverride(p, "local", ["mcp", "servers"], "fs")).toBeNull();
    const skipped = [layer("default"), layer("user"), layer("project"), layer("projectLocal", true, "oops")];
    expect(namedOverride(provenance({ raws, layers: skipped }), "user", ["mcp", "servers"], "fs")).toBe("project");
  });

  it("finds a higher layer's check with the same id", () => {
    const p = provenance({ raws });
    expect(idOverride(p, "user", ["verification", "checks"], "unit")).toBe("project");
    expect(idOverride(p, "user", ["verification", "checks"], "lint")).toBeNull();
  });

  it("lists a layer's table names sorted", () => {
    expect(tableNames(raws.user, ["mcp", "servers"])).toEqual(["fs", "git"]);
    expect(tableNames(undefined, ["mcp", "servers"])).toEqual([]);
  });
});

describe("layer state", () => {
  it("reports active layers and skip reasons", () => {
    const layers = [layer("user"), layer("project", true, "unknown effort"), layer("projectLocal", false)];
    const p = provenance({ layers });
    expect(layerActive(p, "user")).toBe(true);
    expect(layerActive(p, "project")).toBe(false);
    expect(layerActive(p, "projectLocal")).toBe(false);
    expect(layerError(p, "project")).toBe("unknown effort");
    expect(layerError(p, "local")).toBeNull();
  });
});
