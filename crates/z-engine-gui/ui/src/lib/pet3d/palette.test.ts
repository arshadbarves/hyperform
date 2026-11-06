import { afterEach, describe, expect, it, vi } from "vitest";
import { Color } from "three";
import { bodyColor, mixColors, petAlpha, petColor, refreshPalette, toneColor } from "./palette";

function stubTokens(tokens: Record<string, string>) {
  vi.stubGlobal("document", { documentElement: {} });
  vi.stubGlobal("getComputedStyle", () => ({ getPropertyValue: (name: string) => tokens[name] ?? "" }));
  refreshPalette();
}

afterEach(() => {
  vi.unstubAllGlobals();
  refreshPalette();
});

describe("palette", () => {
  it("falls back to the tokens.css values where no stylesheet is loaded", () => {
    expect(bodyColor("mint").getHexString()).toBe("b8f0d8");
    expect(petColor("leafLight").getHexString()).toBe("9ae3ad");
    expect(petColor("rim").getHexString()).toBe("ffffff");
    expect(petAlpha("rim")).toBeCloseTo(0.42);
    expect(petAlpha("gold")).toBe(1);
  });

  it("reads the tokens from the document, until refreshed", () => {
    stubTokens({ "--pet-sky": " #123456", "--pet-rim": "rgb(10 20 30 / 50%)" });
    expect(bodyColor("sky").getHexString()).toBe("123456");
    expect(petColor("rim").getHexString()).toBe("0a141e");
    expect(petAlpha("rim")).toBe(0.5);

    vi.stubGlobal("getComputedStyle", () => ({ getPropertyValue: () => "#abc" }));
    expect(bodyColor("sky").getHexString()).toBe("123456");
    refreshPalette();
    expect(bodyColor("sky").getHexString()).toBe("aabbcc");
  });

  it("ignores a token it cannot read", () => {
    stubTokens({ "--pet-scarf": "color-mix(in srgb, red, blue)" });
    expect(petColor("scarf").getHexString()).toBe("ff6b6b");
  });

  it("tints the aura by tone, and a quiet pet by its body", () => {
    expect(toneColor("working", "pearl").getHexString()).toBe("0a84ff");
    expect(toneColor("attention", "pearl").getHexString()).toBe("ff9f0a");
    expect(toneColor("ok", "pearl").getHexString()).toBe("30d158");
    expect(toneColor("danger", "pearl").getHexString()).toBe("ff453a");
    expect(toneColor("quiet", "lilac").getHexString()).toBe("d9c8ff");
  });

  it("hands out copies, so a material cannot change the palette", () => {
    petColor("gold").set(0x000000);
    expect(petColor("gold").getHexString()).toBe("ffd76a");
  });

  it("mixes in sRGB, as color-mix does", () => {
    expect(mixColors(new Color(0xffffff), new Color(0x000000), 0.5).getHexString()).toBe("808080");
    expect(mixColors(new Color(0xff0000), new Color(0x0000ff), 1).getHexString()).toBe("ff0000");
  });
});
