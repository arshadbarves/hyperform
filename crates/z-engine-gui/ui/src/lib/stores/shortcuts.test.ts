import { describe, expect, it } from "vitest";
import { shortcutFor, type KeyInput } from "./shortcuts";

function key(k: string, mods: Partial<KeyInput> = {}): KeyInput {
  return { key: k, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...mods };
}

describe("shortcutFor", () => {
  it("maps the documented ⌘/Ctrl shortcuts", () => {
    expect(shortcutFor(key("k", { metaKey: true }))).toBe("palette");
    expect(shortcutFor(key("n", { ctrlKey: true }))).toBe("newChat");
    expect(shortcutFor(key("b", { metaKey: true }))).toBe("toggleSidebar");
    expect(shortcutFor(key("d", { metaKey: true }))).toBe("toggleChanges");
    expect(shortcutFor(key(",", { metaKey: true }))).toBe("settings");
  });

  it("ignores case, so Caps Lock does not break shortcuts", () => {
    expect(shortcutFor(key("K", { metaKey: true }))).toBe("palette");
  });

  it("needs the modifier and leaves other chords alone", () => {
    expect(shortcutFor(key("k"))).toBeNull();
    expect(shortcutFor(key("k", { metaKey: true, altKey: true }))).toBeNull();
    expect(shortcutFor(key("k", { metaKey: true, shiftKey: true }))).toBeNull();
    expect(shortcutFor(key("z", { metaKey: true }))).toBeNull();
  });
});
