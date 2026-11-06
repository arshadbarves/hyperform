import { describe, expect, it } from "vitest";
import { credentialKey, keyHintText, keyStatus, searchKeyBucket } from "./credentials";

describe("credentialKey", () => {
  it("names buckets like the config crate", () => {
    expect(credentialKey("https://openrouter.ai/api/v1")).toBe("openrouter");
    expect(credentialKey("https://opencode.ai/zen/v1")).toBe("opencode");
    expect(credentialKey("https://api.anthropic.com/v1/")).toBe("anthropic");
    expect(credentialKey("https://api.openai.com/v1")).toBe("openai");
    expect(credentialKey("http://user:pw@LocalHost:11434/v1")).toBe("custom:localhost:11434");
    expect(credentialKey("https://evil.example/?openrouter.ai")).toBe("custom:evil.example");
    expect(credentialKey("http://[::1]:8080/v1")).toBe("custom:[::1]:8080");
  });

  it("does not share a key with look-alike hosts", () => {
    expect(credentialKey("https://notopenrouter.ai/v1")).toBe("custom:notopenrouter.ai");
    expect(credentialKey("https://eu.openrouter.ai/v1")).toBe("openrouter");
    expect(credentialKey("https://openai.com.evil/v1")).toBe("custom:openai.com.evil");
  });

  it("keeps GitHub.com Copilot in one bucket and each Enterprise host in its own", () => {
    expect(credentialKey("https://api.githubcopilot.com")).toBe("github-copilot");
    expect(credentialKey("https://githubcopilot.com/")).toBe("github-copilot");
    expect(credentialKey("https://api.individual.githubcopilot.com/chat")).toBe("github-copilot");
    expect(credentialKey("https://notgithubcopilot.com")).toBe("custom:notgithubcopilot.com");
    expect(credentialKey("https://copilot-api.acme.ghe.com")).toBe("custom:copilot-api.acme.ghe.com");
  });
});

describe("key status", () => {
  it("defaults to no key and shows the saved hint", () => {
    const creds = { openrouter: { hasKey: true, hint: "abcd" } };
    expect(keyStatus(creds, "openrouter")).toEqual({ hasKey: true, hint: "abcd" });
    expect(keyStatus(creds, "anthropic")).toEqual({ hasKey: false, hint: null });
    expect(keyStatus(creds, null).hasKey).toBe(false);
    expect(keyHintText({ hasKey: true, hint: "abcd" })).toBe("Saved (••••abcd)");
  });

  it("maps search backends to their key buckets", () => {
    expect(searchKeyBucket("brave")).toBe("brave");
    expect(searchKeyBucket("exa")).toBe("exa");
    expect(searchKeyBucket("searxng")).toBeNull();
    expect(searchKeyBucket("none")).toBeNull();
  });
});
