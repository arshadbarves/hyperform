import { describe, expect, it } from "vitest";
import { catalogContextWindow, catalogForPicker, groupModels, isFreeModel, lookupModel, type CatalogData } from "./catalog";
import type { ModelInfo } from "./protocol/ModelInfo";
import type { Pricing } from "./protocol/Pricing";

type ModelOptions = { reasoning?: boolean; deprecated?: boolean; pricing?: Pricing | null };

function model(provider: string, id: string, name: string, options: ModelOptions = {}): ModelInfo {
  const { reasoning = false, deprecated = false, pricing = null } = options;
  return { id, name, provider, contextWindow: 0, maxOutput: 0, pricing, tools: true, vision: false, reasoning, deprecated };
}

const priced = (input: number): Pricing => ({ input, output: input * 5, cacheRead: input / 10, cacheWrite: input });

const ZEN_URL = "https://opencode.ai/zen/v1";

const sample: CatalogData = [
  model("openrouter", "anthropic/claude-sonnet-4", "Claude Sonnet 4", { reasoning: true }),
  model("anthropic", "claude-sonnet-4-5", "Claude Sonnet 4.5", { reasoning: true }),
  model("openai", "gpt-4o", "GPT-4o"),
  model("opencode", "big-pickle", "Big Pickle", { reasoning: true }),
];

describe("catalogForPicker", () => {
  it("shows OpenRouter while it is active and connected", () => {
    const out = catalogForPicker(sample, "https://openrouter.ai/api/v1", true);
    expect(out.map((m) => m.id)).toEqual(["anthropic/claude-sonnet-4"]);
  });

  it("shows OpenCode while it is active without an API key", () => {
    const out = catalogForPicker(sample, ZEN_URL, false);
    expect(out.map((m) => m.name)).toEqual(["Big Pickle"]);
  });

  it("shows native Anthropic models for the Anthropic endpoint", () => {
    expect(catalogForPicker(sample, "https://api.anthropic.com", true).map((m) => m.id)).toEqual(["claude-sonnet-4-5"]);
  });

  it("hides OpenRouter models after its API key is disconnected", () => {
    expect(catalogForPicker(sample, "https://openrouter.ai/api/v1", false)).toEqual([]);
  });

  it("returns empty when the catalog is missing or lacks the active provider", () => {
    expect(catalogForPicker(null, ZEN_URL, false)).toEqual([]);
    expect(catalogForPicker([sample[2]], ZEN_URL, false)).toEqual([]);
  });

  const zen: CatalogData = [
    model("opencode", "claude-sonnet-4-5", "Claude Sonnet 4.5", { pricing: priced(3) }),
    model("opencode", "big-pickle", "Big Pickle", { pricing: priced(0) }),
    model("opencode", "glm-4.7-free", "GLM-4.7 Free", { pricing: priced(0), deprecated: true }),
    model("opencode", "gpt-5", "GPT-5", { pricing: priced(1.25) }),
    model("opencode", "mimo-free", "MiMo Free"),
  ];

  it("lists only free, current Zen models without a key", () => {
    expect(catalogForPicker(zen, ZEN_URL, false).map((m) => m.id)).toEqual(["big-pickle", "mimo-free"]);
  });

  it("lists paid Zen models too with a key, free ones first in catalog order", () => {
    expect(catalogForPicker(zen, ZEN_URL, true).map((m) => m.id)).toEqual([
      "big-pickle",
      "mimo-free",
      "claude-sonnet-4-5",
      "gpt-5",
    ]);
  });

  it("hides deprecated models for other providers too", () => {
    const openrouter: CatalogData = [
      model("openrouter", "acme/retired", "Retired", { deprecated: true }),
      model("openrouter", "acme/current", "Current"),
    ];
    expect(catalogForPicker(openrouter, "https://openrouter.ai/api/v1", true).map((m) => m.id)).toEqual(["acme/current"]);
  });
});

describe("isFreeModel", () => {
  it("counts only OpenCode Zen models with no input price", () => {
    expect(isFreeModel(model("opencode", "big-pickle", "Big Pickle", { pricing: priced(0) }))).toBe(true);
    expect(isFreeModel(model("opencode", "unpriced", "Unpriced"))).toBe(true);
    expect(isFreeModel(model("opencode", "gpt-5", "GPT-5", { pricing: priced(1.25) }))).toBe(false);
  });

  it("is false for OpenRouter and GitHub Copilot models priced at 0", () => {
    expect(isFreeModel(model("openrouter", "acme/free", "Free", { pricing: priced(0) }))).toBe(false);
    expect(isFreeModel(model("github-copilot", "gpt-4o", "GPT-4o", { pricing: priced(0) }))).toBe(false);
  });
});

describe("groupModels", () => {
  it("groups by provider name and filters by id, name or provider", () => {
    expect(groupModels(sample, "").map((g) => [g.provider, g.items.length])).toEqual([
      ["Anthropic", 1],
      ["OpenAI", 1],
      ["OpenCode Zen", 1],
      ["OpenRouter", 1],
    ]);
    expect(groupModels(sample, "sonnet").flatMap((g) => g.items.map((m) => m.id))).toEqual([
      "claude-sonnet-4-5",
      "anthropic/claude-sonnet-4",
    ]);
    expect(groupModels(sample, "zen").map((g) => g.provider)).toEqual(["OpenCode Zen"]);
    expect(groupModels(sample, "", 0).every((g) => g.items.length === 0)).toBe(true);
  });
});

describe("lookupModel", () => {
  it("resolves exact, vendor-prefixed and case-insensitive ids like the engine", () => {
    expect(lookupModel(sample, "gpt-4o")?.providerId).toBe("openai");
    expect(lookupModel(sample, "openai/gpt-4o")?.id).toBe("gpt-4o");
    expect(lookupModel(sample, "claude-sonnet-4")?.id).toBe("anthropic/claude-sonnet-4");
    expect(lookupModel(sample, "GPT-4O")?.model.name).toBe("GPT-4o");
    expect(lookupModel(sample, "no-such-model")).toBeNull();
    expect(lookupModel(null, "gpt-4o")).toBeNull();
  });
});

describe("catalogContextWindow", () => {
  const windowed = (provider: string, id: string, contextWindow: number): ModelInfo => ({
    ...model(provider, id, id),
    contextWindow,
  });
  const catalog: CatalogData = [
    windowed("openai", "gpt-4o", 128_000),
    windowed("openai", "gpt-5-mini", 400_000),
    windowed("github-copilot", "gpt-4o", 64_000),
    windowed("github-copilot", "gpt-5-mini", 0),
  ];

  it("prefers the endpoint's own entry, like the engine", () => {
    expect(catalogContextWindow(catalog, "gpt-4o", "https://api.githubcopilot.com")).toBe(64_000);
    expect(catalogContextWindow(catalog, "gpt-4o", "https://copilot-api.acme.ghe.com")).toBe(64_000);
    expect(catalogContextWindow(catalog, "gpt-4o", "https://api.openai.com/v1")).toBe(128_000);
  });

  it("falls back to the preferred entry when the provider's has no window or the endpoint has no provider", () => {
    expect(catalogContextWindow(catalog, "gpt-5-mini", "https://api.githubcopilot.com")).toBe(400_000);
    expect(catalogContextWindow(catalog, "gpt-4o", "https://openrouter.ai/api/v1")).toBe(128_000);
    expect(catalogContextWindow(catalog, "gpt-4o", "http://localhost:11434/v1")).toBe(128_000);
    expect(catalogContextWindow(catalog, "unknown", "https://api.githubcopilot.com")).toBe(0);
    expect(catalogContextWindow(null, "gpt-4o", "https://api.githubcopilot.com")).toBe(0);
  });
});
