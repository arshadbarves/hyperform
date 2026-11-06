import { describe, expect, it } from "vitest";
import type { ProviderSettings } from "../../protocol/config/ProviderSettings";
import { connectFormDefaults, PROVIDERS } from "../../providers";
import { instructionSlots, isTruncated, joinPath, otherInstructionFiles, samePath } from "./instructions";
import { connectFormError, connectWrites, normalizeBaseUrl } from "./providerWrites";

function preset(id: string) {
  const found = PROVIDERS.find((p) => p.id === id);
  if (!found) throw new Error(`missing provider ${id}`);
  return found;
}

const openrouter: ProviderSettings = {
  kind: "auto",
  base_url: "https://openrouter.ai/api/v1",
  headers: {},
  cache_control: null,
};

describe("provider presets to settings writes", () => {
  it("writes the native Anthropic kind, endpoint and model", () => {
    const form = connectFormDefaults(preset("anthropic"), { model: "openrouter/auto", provider: openrouter });
    expect(connectWrites(form, openrouter)).toEqual([
      { op: "set", keyPath: ["provider", "kind"], value: "anthropic" },
      { op: "set", keyPath: ["provider", "base_url"], value: "https://api.anthropic.com" },
      { op: "set", keyPath: ["model", "main"], value: "claude-sonnet-4-5" },
    ]);
  });

  it("switches to keyless OpenCode Zen as an OpenAI-compatible endpoint", () => {
    const form = connectFormDefaults(preset("opencode"), { model: "x", provider: openrouter });
    expect(connectWrites(form, openrouter).slice(0, 2)).toEqual([
      { op: "set", keyPath: ["provider", "kind"], value: "openai_chat" },
      { op: "set", keyPath: ["provider", "base_url"], value: "https://opencode.ai/zen/v1" },
    ]);
    expect(connectFormError(preset("opencode"), form, "", false)).toBeNull();
  });

  it("writes headers and caching only when the form changed them", () => {
    const form = { model: "m", baseUrl: "http://localhost:1234/v1/", kind: "auto" as const, headers: { "X-Team": "a" }, cacheControl: false };
    expect(connectWrites(form, openrouter)).toEqual([
      { op: "set", keyPath: ["provider", "kind"], value: "auto" },
      { op: "set", keyPath: ["provider", "base_url"], value: "http://localhost:1234/v1" },
      { op: "set", keyPath: ["model", "main"], value: "m" },
      { op: "set", keyPath: ["provider", "headers"], value: { "X-Team": "a" } },
      { op: "set", keyPath: ["provider", "cache_control"], value: false },
    ]);
    const current = { ...openrouter, headers: { "X-Team": "a" }, cache_control: true };
    const cleared = connectWrites({ ...form, headers: {}, cacheControl: null }, current);
    expect(cleared.slice(3)).toEqual([
      { op: "remove", keyPath: ["provider", "headers"] },
      { op: "remove", keyPath: ["provider", "cache_control"] },
    ]);
  });

  it("explains what the form still needs", () => {
    const custom = connectFormDefaults(preset("custom"), { provider: openrouter });
    expect(connectFormError(preset("custom"), custom, "k", false)).toContain("base URL");
    expect(connectFormError(preset("custom"), { ...custom, baseUrl: "proxy.local" }, "k", false)).toContain("http");
    expect(connectFormError(preset("lmstudio"), { ...connectFormDefaults(preset("lmstudio"), {}) }, "", false)).toContain("model");
    const anthropic = connectFormDefaults(preset("anthropic"), {});
    expect(connectFormError(preset("anthropic"), anthropic, "", false)).toContain("API key");
    expect(connectFormError(preset("anthropic"), anthropic, "", true)).toBeNull();
    expect(normalizeBaseUrl(" https://x/v1// ")).toBe("https://x/v1");
  });

  it("asks Copilot for a sign-in, never a key", () => {
    const copilot = connectFormDefaults(preset("github-copilot"), {});
    expect(connectFormError(preset("github-copilot"), copilot, "typed", false)).toBe("Sign in with GitHub to connect.");
    expect(connectFormError(preset("github-copilot"), copilot, "", true)).toBeNull();
    expect(connectFormError(preset("github-copilot"), { ...copilot, baseUrl: "" }, "", true)).toContain("Enterprise domain");
  });
});

describe("instruction files", () => {
  const files = [
    { scope: "user" as const, path: "/home/me/.config/hyperform/AGENTS.md", content: "u" },
    { scope: "project" as const, path: "/work/app/AGENTS.md", content: "p" },
    { scope: "project" as const, path: "/work/app/CLAUDE.md", content: "c" },
  ];

  it("maps the three editable files and finds their current content", () => {
    const slots = instructionSlots("/home/me/.config/hyperform", "/work/app", files);
    expect(slots.map((s) => [s.scope, s.path, s.file?.content ?? null])).toEqual([
      ["user", "/home/me/.config/hyperform/AGENTS.md", "u"],
      ["project", "/work/app/AGENTS.md", "p"],
      ["local", "/work/app/AGENTS.local.md", null],
    ]);
    expect(otherInstructionFiles(files, slots).map((f) => f.path)).toEqual(["/work/app/CLAUDE.md"]);
    expect(instructionSlots(null, null, files)).toEqual([]);
  });

  it("recognises content the loader cut at 64 KiB", () => {
    expect(isTruncated("text\n\n[Truncated: this file is larger than 64 KiB.]")).toBe(true);
    expect(isTruncated("text")).toBe(false);
  });

  it("joins and compares paths across separators", () => {
    expect(joinPath("C:\\Users\\me\\hyperform\\", "AGENTS.md")).toBe("C:\\Users\\me\\hyperform\\AGENTS.md");
    expect(joinPath("/work/app/", "AGENTS.md")).toBe("/work/app/AGENTS.md");
    expect(samePath("C:\\Work\\App\\AGENTS.md", "c:/work/app/AGENTS.md")).toBe(true);
    expect(samePath("/a/AGENTS.md", "/a/AGENTS.local.md")).toBe(false);
  });
});
