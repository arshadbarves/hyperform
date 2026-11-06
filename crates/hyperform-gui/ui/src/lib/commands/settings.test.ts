import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as settings from "./settings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

beforeEach(() => vi.clearAllMocks());

const project = { scope: "project" as const, projectRoot: "/work/app" };
const user = { scope: "user" as const, projectRoot: null };
const server = {
  command: "npx",
  args: [],
  env: {},
  cwd: null,
  url: null,
  headers: {},
  enabled: true,
  disabled_tools: [],
  timeout_secs: 60,
};
const hook = { matcher: "Bash", command: "./x.sh", timeout_secs: 60 };

describe("v2 settings IPC", () => {
  it.each<[string, () => Promise<unknown>, string, Record<string, unknown> | undefined]>([
    ["appInfo", () => settings.appInfo(), "app_info", undefined],
    ["getSettings", () => settings.getSettings(null), "get_settings", { projectRoot: null }],
    ["getLayer", () => settings.getLayer("local", "/work/app"), "get_layer", { scope: "local", projectRoot: "/work/app" }],
    [
      "setSetting",
      () => settings.setSetting(project, ["model", "main"], "m"),
      "set_setting",
      { scope: "project", projectRoot: "/work/app", keyPath: ["model", "main"], value: "m" },
    ],
    [
      "removeSetting",
      () => settings.removeSetting(user, ["model", "fast"]),
      "remove_setting",
      { scope: "user", projectRoot: null, keyPath: ["model", "fast"] },
    ],
    [
      "addPermissionRule",
      () => settings.addPermissionRule(project, "deny", "Read(./.env)"),
      "add_permission_rule",
      { scope: "project", projectRoot: "/work/app", kind: "deny", rule: "Read(./.env)" },
    ],
    [
      "removePermissionRule",
      () => settings.removePermissionRule(user, "allow", "Read"),
      "remove_permission_rule",
      { scope: "user", projectRoot: null, kind: "allow", rule: "Read" },
    ],
    [
      "setMcpServer",
      () => settings.setMcpServer(user, "fs", server),
      "set_mcp_server",
      { scope: "user", projectRoot: null, name: "fs", server },
    ],
    [
      "removeMcpServer",
      () => settings.removeMcpServer(project, "fs"),
      "remove_mcp_server",
      { scope: "project", projectRoot: "/work/app", name: "fs" },
    ],
    ["testMcpServer", () => settings.testMcpServer(server, "/work/app"), "test_mcp_server", { server, projectRoot: "/work/app" }],
    ["featureCatalog", () => settings.featureCatalog(), "feature_catalog", undefined],
    ["testDecisionModel", () => settings.testDecisionModel(null), "test_decision_model", { projectRoot: null }],
    ["decisionModelStatus", () => settings.decisionModelStatus(null), "decision_model_status", { projectRoot: null }],
    [
      "downloadDecisionModel",
      () => settings.downloadDecisionModel("/work/app"),
      "download_decision_model",
      { projectRoot: "/work/app" },
    ],
    [
      "cancelDecisionModelDownload",
      () => settings.cancelDecisionModelDownload(),
      "cancel_decision_model_download",
      undefined,
    ],
    ["removeDecisionModel", () => settings.removeDecisionModel(null), "remove_decision_model", { projectRoot: null }],
    [
      "setHooks",
      () => settings.setHooks(project, "PreToolUse", [hook]),
      "set_hooks",
      { scope: "project", projectRoot: "/work/app", event: "PreToolUse", hooks: [hook] },
    ],
    ["credentialStatus", () => settings.credentialStatus(), "credential_status", undefined],
    [
      "saveApiKey",
      () => settings.saveApiKey("https://api.anthropic.com", null),
      "save_api_key",
      { baseUrl: "https://api.anthropic.com", key: null },
    ],
    [
      "copilotLoginStart",
      () => settings.copilotLoginStart("https://api.githubcopilot.com"),
      "copilot_login_start",
      { baseUrl: "https://api.githubcopilot.com" },
    ],
    [
      "copilotLoginPoll",
      () => settings.copilotLoginPoll("https://copilot-api.acme.ghe.com", "dc-1"),
      "copilot_login_poll",
      { baseUrl: "https://copilot-api.acme.ghe.com", deviceCode: "dc-1" },
    ],
    ["saveSearchKey", () => settings.saveSearchKey("brave", "k"), "save_search_key", { backend: "brave", key: "k" }],
    ["trustStatus", () => settings.trustStatus("/work/app"), "trust_status", { projectRoot: "/work/app" }],
    ["setTrust", () => settings.setTrust("/work/app", true), "set_trust", { projectRoot: "/work/app", trusted: true }],
    ["listExtensions", () => settings.listExtensions(null), "list_extensions", { projectRoot: null }],
    ["readExtensionFile", () => settings.readExtensionFile("/p/a.md"), "read_extension_file", { path: "/p/a.md" }],
    [
      "writeExtensionFile",
      () => settings.writeExtensionFile(null, "output-styles", "terse", "---\n---\n"),
      "write_extension_file",
      { projectRoot: null, kind: "output-styles", name: "terse", content: "---\n---\n" },
    ],
    ["deleteExtensionFile", () => settings.deleteExtensionFile("/p/a.md"), "delete_extension_file", { path: "/p/a.md" }],
    ["listInstructionFiles", () => settings.listInstructionFiles("/work/app"), "list_instruction_files", { projectRoot: "/work/app" }],
    [
      "writeInstructionFile",
      () => settings.writeInstructionFile("/work/app/AGENTS.md", "# Notes"),
      "write_instruction_file",
      { path: "/work/app/AGENTS.md", content: "# Notes" },
    ],
  ])("%s calls %s with camelCase arguments", async (_name, call, command, args) => {
    await call();
    if (args === undefined) expect(invoke).toHaveBeenCalledWith(command);
    else expect(invoke).toHaveBeenCalledWith(command, args);
  });
});
