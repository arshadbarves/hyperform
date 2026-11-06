import { describe, expect, it } from "vitest";
import { formatArgs, formatKeyValues, formatList, optionalText, parseArgs, parseKeyValues, parseList } from "./formText";
import { LIMITS, parseNumber } from "./limits";
import { sameJson, settingWrite, toTomlValue } from "./tomlValue";

describe("toTomlValue", () => {
  it("drops nulls, undefined and non-finite numbers, which TOML cannot hold", () => {
    expect(toTomlValue(null)).toBeUndefined();
    expect(toTomlValue(Number.NaN)).toBeUndefined();
    expect(
      toTomlValue({ id: "unit", cwd: null, label: undefined, timeout_secs: 600, tags: ["a", null, "b"], nested: { x: null } }),
    ).toEqual({ id: "unit", timeout_secs: 600, tags: ["a", "b"], nested: {} });
  });

  it("keeps empty lists, which still override lower layers", () => {
    expect(toTomlValue([])).toEqual([]);
    expect(toTomlValue(false)).toBe(false);
    expect(toTomlValue(0)).toBe(0);
  });
});

describe("settingWrite", () => {
  it("sets values and removes unset optional keys", () => {
    expect(settingWrite(["model", "fast"], "m")).toEqual({ op: "set", keyPath: ["model", "fast"], value: "m" });
    expect(settingWrite(["model", "fast"], null)).toEqual({ op: "remove", keyPath: ["model", "fast"] });
    expect(settingWrite(["model", "fallbacks"], [])).toEqual({ op: "set", keyPath: ["model", "fallbacks"], value: [] });
  });
});

describe("sameJson", () => {
  it("compares structure regardless of key order and treats null like missing", () => {
    expect(sameJson({ a: 1, b: [1, { c: 2 }] }, { b: [1, { c: 2 }], a: 1 })).toBe(true);
    expect(sameJson({ a: 1 }, { a: 2 })).toBe(false);
    expect(sameJson([1], [1, 2])).toBe(false);
    expect(sameJson(null, undefined)).toBe(true);
  });
});

describe("form text", () => {
  it("parses lists from commas and lines without blanks or repeats", () => {
    expect(parseList("a, b\n\nc, a,")).toEqual(["a", "b", "c"]);
    expect(formatList(["a", "b"])).toBe("a, b");
    expect(optionalText("  ")).toBeNull();
    expect(optionalText(" x ")).toBe("x");
  });

  it("splits arguments like a shell and round-trips them", () => {
    expect(parseArgs(`-y "@scope/server name" '.' a\\ b ""`)).toEqual({
      args: ["-y", "@scope/server name", ".", "a b", ""],
      error: null,
    });
    expect(parseArgs(`"open`).error).toContain("quote");
    const args = ["-y", "with space", `say "hi"`, "back\\slash", ""];
    expect(parseArgs(formatArgs(args)).args).toEqual(args);
  });

  it("parses KEY=value and Name: value lines", () => {
    expect(parseKeyValues("A=1\n# note\nB = x=y\n", "=")).toEqual({ values: { A: "1", B: "x=y" }, error: null });
    expect(parseKeyValues("Authorization: Bearer a:b", ":").values).toEqual({ Authorization: "Bearer a:b" });
    expect(parseKeyValues("=oops", "=").error).toContain("Line 1");
    expect(formatKeyValues({ A: "1" }, "=")).toBe("A=1");
    expect(formatKeyValues({ A: "1" }, ":")).toBe("A: 1");
  });
});

describe("parseNumber", () => {
  it("accepts values inside the loader's clamp range", () => {
    expect(parseNumber("256", LIMITS.maxOutputTokens)).toEqual({ ok: true, value: 256 });
    expect(parseNumber("99", LIMITS.compactAtPercent)).toEqual({ ok: true, value: 99 });
    expect(parseNumber("0", LIMITS.maxContinuations)).toEqual({ ok: true, value: 0 });
    expect(parseNumber("2.5", LIMITS.sessionCostCapUsd)).toEqual({ ok: true, value: 2.5 });
  });

  it("rejects values the loader would clamp, fractions and blanks", () => {
    expect(parseNumber("255", LIMITS.maxOutputTokens)).toEqual({ ok: false, error: "Enter a whole number from 256 to 200000." });
    expect(parseNumber("100", LIMITS.compactAtPercent).ok).toBe(false);
    expect(parseNumber("11", LIMITS.maxContinuations).ok).toBe(false);
    expect(parseNumber("0", LIMITS.maxConcurrent)).toEqual({ ok: false, error: "Enter a whole number of at least 1." });
    expect(parseNumber("1.5", LIMITS.maxTurns).ok).toBe(false);
    expect(parseNumber("-1", LIMITS.sessionCostCapUsd).ok).toBe(false);
    expect(parseNumber("", LIMITS.maxTurns).ok).toBe(false);
  });

  it("treats blank as unset for optional settings", () => {
    expect(parseNumber(" ", LIMITS.contextWindow, true)).toEqual({ ok: true, value: null });
  });
});
