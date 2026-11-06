import { describe, expect, it } from "vitest";
import { emptyLspForm, lspFormFrom, lspNameError, lspServerFrom, normalizeLspServer } from "./lspForm";
import {
  emptyMcpForm,
  mcpFormFrom,
  mcpNameError,
  mcpServerFrom,
  normalizeMcpServer,
  serverSummary,
  transportError,
} from "./mcpForm";

describe("MCP servers", () => {
  it("fills the loader's defaults for a sparse layer entry", () => {
    expect(normalizeMcpServer({ command: "npx", args: ["-y", 3] })).toEqual({
      command: "npx",
      args: ["-y"],
      env: {},
      cwd: null,
      url: null,
      headers: {},
      enabled: true,
      disabled_tools: [],
      timeout_secs: 60,
    });
  });

  it("needs exactly one of command and URL", () => {
    const base = normalizeMcpServer({});
    expect(transportError(base)).toContain("command or a URL");
    expect(transportError({ ...base, command: "npx", url: "http://x" })).toContain("only one");
    expect(transportError({ ...base, url: "http://x" })).toBeNull();
  });

  it("builds a stdio server from the form and back", () => {
    const form = {
      ...emptyMcpForm(),
      name: "fs",
      command: "npx",
      args: `-y @modelcontextprotocol/server-filesystem "my dir"`,
      env: "TOKEN=abc",
      url: "ignored for stdio",
      disabledTools: "write_file, move_file",
      timeout: "30",
    };
    const built = mcpServerFrom(form);
    if (!("server" in built)) throw new Error(built.error);
    expect(built.server).toMatchObject({
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem", "my dir"],
      env: { TOKEN: "abc" },
      url: null,
      disabled_tools: ["write_file", "move_file"],
      timeout_secs: 30,
    });
    expect(mcpFormFrom("fs", built.server)).toMatchObject({ transport: "stdio", command: "npx", env: "TOKEN=abc" });
    expect(serverSummary(built.server)).toBe(`npx -y @modelcontextprotocol/server-filesystem "my dir"`);
  });

  it("builds an HTTP server with headers only", () => {
    const form = { ...emptyMcpForm(), transport: "http" as const, url: "https://mcp.example/v1", headers: "Authorization: Bearer t", command: "npx" };
    const built = mcpServerFrom(form);
    if (!("server" in built)) throw new Error(built.error);
    expect(built.server).toMatchObject({ command: null, url: "https://mcp.example/v1", headers: { Authorization: "Bearer t" } });
    expect(mcpFormFrom("x", built.server).transport).toBe("http");
  });

  it("reports form errors", () => {
    expect(mcpServerFrom({ ...emptyMcpForm(), command: "npx", timeout: "0" })).toEqual({ error: expect.stringContaining("Timeout") });
    expect(mcpServerFrom({ ...emptyMcpForm(), command: "npx", args: `"open` })).toEqual({ error: expect.stringContaining("Arguments") });
    expect(mcpServerFrom(emptyMcpForm())).toEqual({ error: expect.stringContaining("command or a URL") });
  });

  it("validates names used in mcp__server__tool rules", () => {
    expect(mcpNameError("github", [])).toBeNull();
    expect(mcpNameError("", [])).toContain("name");
    expect(mcpNameError("my__server", [])).toContain("single _");
    expect(mcpNameError("git hub", [])).toContain("letters");
    expect(mcpNameError("github", ["github"])).toContain("already exists");
  });
});

describe("LSP servers", () => {
  it("round-trips a server and strips dots from extensions", () => {
    const form = { ...emptyLspForm(), name: "rust", command: "rust-analyzer", extensions: ".rs", rootMarkers: "Cargo.toml" };
    const built = lspServerFrom(form);
    if (!("server" in built)) throw new Error(built.error);
    expect(built.server).toEqual({ command: "rust-analyzer", args: [], extensions: ["rs"], root_markers: ["Cargo.toml"], enabled: true });
    expect(lspFormFrom("rust", built.server)).toMatchObject({ extensions: "rs", rootMarkers: "Cargo.toml" });
    expect(normalizeLspServer({ command: "x", enabled: false })).toMatchObject({ command: "x", enabled: false, args: [] });
  });

  it("needs a command, extensions and a unique name", () => {
    expect(lspServerFrom(emptyLspForm())).toEqual({ error: expect.stringContaining("command") });
    expect(lspServerFrom({ ...emptyLspForm(), command: "x" })).toEqual({ error: expect.stringContaining("extensions") });
    expect(lspNameError("rust", ["rust"])).toContain("already exists");
    expect(lspNameError("", [])).toContain("Enter");
  });
});
