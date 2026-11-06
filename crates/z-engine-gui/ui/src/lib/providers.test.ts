import { describe, expect, it } from "vitest";
import type { ProviderSettings } from "./protocol/config/ProviderSettings";
import {
  canSubmitProviderConnect,
  connectFormDefaults,
  copilotBaseUrl,
  copilotEnterpriseDomain,
  detectProviderId,
  engineCatalogProvider,
  isProviderConnected,
  normalizeCopilotDomain,
  PROVIDERS,
  requiresApiKey,
} from "./providers";

function preset(id: string) {
  const found = PROVIDERS.find((p) => p.id === id);
  if (!found) throw new Error(`missing provider ${id}`);
  return found;
}

const live = (base_url: string, overrides: Partial<ProviderSettings> = {}): ProviderSettings => ({
  kind: "auto",
  base_url,
  headers: {},
  cache_control: null,
  ...overrides,
});

describe("OpenCode Zen keyless connect", () => {
  it("detects the Zen gateway", () => {
    expect(detectProviderId("https://opencode.ai/zen/v1")).toBe("opencode");
  });

  it("does not require an API key to connect", () => {
    expect(requiresApiKey(preset("opencode"))).toBe(false);
    expect(requiresApiKey(preset("ollama"))).toBe(false);
    expect(requiresApiKey(preset("lmstudio"))).toBe(false);
    expect(requiresApiKey(preset("openrouter"))).toBe(true);
    expect(requiresApiKey(preset("anthropic"))).toBe(true);
  });

  it("treats the selected OpenCode provider as connected without a key", () => {
    expect(isProviderConnected(preset("opencode"), true, false)).toBe(true);
    expect(isProviderConnected(preset("opencode"), false, false)).toBe(false);
    expect(isProviderConnected(preset("openrouter"), true, false)).toBe(false);
    expect(isProviderConnected(preset("openrouter"), true, true)).toBe(true);
  });

  it("allows Connect & Set Active with an empty key", () => {
    expect(canSubmitProviderConnect(preset("opencode"), "", false)).toBe(true);
    expect(canSubmitProviderConnect(preset("openrouter"), "", false)).toBe(false);
    expect(canSubmitProviderConnect(preset("openrouter"), "sk-or-v1-x", false)).toBe(true);
  });
});

describe("presets", () => {
  it("reaches Anthropic natively and local servers as OpenAI-compatible endpoints", () => {
    expect(preset("anthropic")).toMatchObject({ kind: "anthropic", baseUrl: "https://api.anthropic.com" });
    expect(preset("lmstudio")).toMatchObject({ kind: "openai_chat", baseUrl: "http://localhost:1234/v1", tag: "Local" });
    expect(preset("custom").kind).toBe("auto");
  });

  it("detects every preset from its own endpoint", () => {
    for (const p of PROVIDERS.filter((p) => p.baseUrl)) expect(detectProviderId(p.baseUrl)).toBe(p.id);
    expect(detectProviderId("")).toBe("openrouter");
    expect(detectProviderId("https://proxy.internal/v1")).toBe("custom");
  });
});

describe("GitHub Copilot", () => {
  it("is an OpenAI-compatible preset with the models.dev Copilot catalog", () => {
    expect(preset("github-copilot")).toMatchObject({
      kind: "openai_chat",
      baseUrl: "https://api.githubcopilot.com",
      defaultModel: "gpt-5-mini",
      catalogProvider: "github-copilot",
      tag: "Sign in",
      auth: "github-device",
    });
    expect(preset("github-copilot").keyUrl).toBeUndefined();
  });

  it("detects GitHub.com and Enterprise Copilot hosts", () => {
    expect(detectProviderId("https://api.githubcopilot.com")).toBe("github-copilot");
    expect(detectProviderId("https://api.individual.githubcopilot.com/")).toBe("github-copilot");
    expect(detectProviderId("https://copilot-api.acme.ghe.com")).toBe("github-copilot");
    expect(detectProviderId("https://proxy.example/copilot-api.x")).toBe("custom");
  });

  it("needs a saved sign-in to count as connected or to submit", () => {
    const copilot = preset("github-copilot");
    expect(requiresApiKey(copilot)).toBe(true);
    expect(isProviderConnected(copilot, true, false)).toBe(false);
    expect(isProviderConnected(copilot, true, true)).toBe(true);
    expect(canSubmitProviderConnect(copilot, "ghp_typed", false)).toBe(false);
    expect(canSubmitProviderConnect(copilot, "", true)).toBe(true);
  });

  it("maps an Enterprise domain to its Copilot API and back", () => {
    expect(copilotBaseUrl(null)).toBe("https://api.githubcopilot.com");
    expect(copilotBaseUrl(" https://ACME.ghe.com/login ")).toBe("https://copilot-api.acme.ghe.com");
    expect(copilotBaseUrl("copilot-api.acme.ghe.com")).toBe("https://copilot-api.acme.ghe.com");
    expect(copilotBaseUrl("  ")).toBe("");
    expect(normalizeCopilotDomain("Acme.GHE.com/")).toBe("acme.ghe.com");
    expect(copilotEnterpriseDomain("https://copilot-api.acme.ghe.com/")).toBe("acme.ghe.com");
    expect(copilotEnterpriseDomain("https://api.githubcopilot.com")).toBeNull();
    expect(copilotEnterpriseDomain("https://copilot-api.")).toBeNull();
  });
});

describe("engineCatalogProvider", () => {
  it("names the models.dev provider the engine prefers, as catalog_provider does", () => {
    expect(engineCatalogProvider("https://api.githubcopilot.com")).toBe("github-copilot");
    expect(engineCatalogProvider("https://copilot-api.acme.ghe.com")).toBe("github-copilot");
    expect(engineCatalogProvider("https://opencode.ai/zen/v1")).toBe("opencode");
    expect(engineCatalogProvider("http://localhost:1234/v1")).toBe("lmstudio");
    expect(engineCatalogProvider("http://localhost:11434/v1")).toBeNull();
    expect(engineCatalogProvider("https://api.deepseek.com/v1")).toBeNull();
    expect(engineCatalogProvider("https://my-proxy.example/v1")).toBeNull();
  });
});

describe("connectFormDefaults", () => {
  const active = { model: "openrouter/free", provider: live("https://openrouter.ai/api/v1") };

  it("switches endpoint, kind and model to the preset when connecting a different provider", () => {
    expect(connectFormDefaults(preset("opencode"), active)).toEqual({
      model: "big-pickle",
      baseUrl: "https://opencode.ai/zen/v1",
      kind: "openai_chat",
      headers: {},
      cacheControl: null,
    });
  });

  it("keeps the live values when reconfiguring the active provider", () => {
    const configured = { model: "openrouter/free", provider: live("https://openrouter.ai/api/v1", { headers: { "X-Title": "z" }, cache_control: false }) };
    expect(connectFormDefaults(preset("openrouter"), configured)).toEqual({
      model: "openrouter/free",
      baseUrl: "https://openrouter.ai/api/v1",
      kind: "auto",
      headers: { "X-Title": "z" },
      cacheControl: false,
    });
  });

  it("keeps a custom endpoint the presets do not know", () => {
    const custom = { model: "local/mix", provider: live("https://proxy.internal/v1", { kind: "anthropic" }) };
    expect(connectFormDefaults(preset("custom"), custom)).toMatchObject({
      model: "local/mix",
      baseUrl: "https://proxy.internal/v1",
      kind: "anthropic",
    });
  });
});
