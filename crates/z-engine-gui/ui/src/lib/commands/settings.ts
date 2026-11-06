import { invoke } from "@tauri-apps/api/core";
import type { Extensions } from "../protocol/config/Extensions";
import type { FeatureSpec } from "../protocol/config/FeatureSpec";
import type { HookConfig } from "../protocol/config/HookConfig";
import type { InstructionFile } from "../protocol/config/InstructionFile";
import type { KeyStatus } from "../protocol/config/KeyStatus";
import type { LoadedSettings } from "../protocol/config/LoadedSettings";
import type { McpServerConfig } from "../protocol/config/McpServerConfig";
import type { RuleKind } from "../protocol/config/RuleKind";
import type { SearchBackend } from "../protocol/config/SearchBackend";

/** A settings file the app writes: user, `.hyperform/settings.toml`, or `settings.local.toml`. */
export type SettingsScope = "user" | "project" | "local";

/** Folder of an extension kind under the user config dir or `.hyperform/`. */
export type ExtensionKind = "agents" | "commands" | "skills" | "rules" | "output-styles";

export interface AppInfo {
  version: string;
  configDir: string;
  dataDir: string;
  /** The window shows native vibrancy (macOS) or Mica (Windows) behind the webview. */
  nativeGlass: boolean;
}

/** One layer file; `raw` is its TOML table as JSON. */
export interface LayerFile {
  path: string;
  exists: boolean;
  raw: unknown;
}

export interface McpTestResult {
  ok: boolean;
  tools: string[];
  resources: number;
  prompts: number;
  error: string | null;
}

/** "Test connection" of the decision model: one known question, asked directly. */
export interface DecisionModelTest {
  ok: boolean;
  latencyMs: number;
  /** The answer's label; a working model answers the probe `yes`. */
  answer: string | null;
  confidence: number | null;
  error: string | null;
  /** The URL the probe went to; null when no connection was made. */
  endpoint: string | null;
  /** The sidecar was started recently, or the native model is still loading. */
  warmingUp: boolean;
  /** `decisions.timeout_ms`, to compare with the latency. */
  timeoutMs: number;
}

/** The native runtime's model for the effective `decisions.checkpoint`. */
export interface DecisionModelStatus {
  checkpoint: string;
  /** A pinned model exists for the checkpoint. */
  available: boolean;
  repo: string | null;
  revision: string | null;
  /** `<data dir>/models/laya/<revision>/`. */
  dir: string | null;
  totalBytes: number;
  downloadedBytes: number;
  /** Every file is on disk with its pinned size. */
  installed: boolean;
  downloading: boolean;
  /** Why the last download stopped, or why there is no model. */
  error: string | null;
  /** This app was built with the native runtime. */
  nativeBuilt: boolean;
}

/** GitHub's device code for Copilot sign-in; `interval` and `expiresIn` are seconds. */
export interface CopilotDeviceCode {
  userCode: string;
  verificationUri: string;
  deviceCode: string;
  interval: number;
  expiresIn: number;
}

/** One poll of the sign-in; on `done` the token is already saved and the settings reloaded. */
export type CopilotLoginPoll =
  | { status: "pending" }
  | { status: "slowDown"; interval: number }
  | { status: "done" }
  | { status: "failed"; reason: string };

export interface TrustStatus {
  trusted: boolean;
  /** What the project would run once trusted, e.g. `hooks`, `mcp servers`, `checks`. */
  projectDefines: string[];
}

/** Where a write lands: `projectRoot` is null for user-level files. */
export interface LayerTarget {
  scope: SettingsScope;
  projectRoot: string | null;
}

export const appInfo = () => invoke<AppInfo>("app_info");

/** Effective settings with every layer; `projectRoot` null loads the user level only. */
export const getSettings = (projectRoot: string | null) =>
  invoke<LoadedSettings>("get_settings", { projectRoot });

export const getLayer = (scope: SettingsScope, projectRoot: string | null) =>
  invoke<LayerFile>("get_layer", { scope, projectRoot });

export const setSetting = (target: LayerTarget, keyPath: string[], value: unknown) =>
  invoke<void>("set_setting", { ...target, keyPath, value });

export const removeSetting = (target: LayerTarget, keyPath: string[]) =>
  invoke<void>("remove_setting", { ...target, keyPath });

export const addPermissionRule = (target: LayerTarget, kind: RuleKind, rule: string) =>
  invoke<void>("add_permission_rule", { ...target, kind, rule });

export const removePermissionRule = (target: LayerTarget, kind: RuleKind, rule: string) =>
  invoke<void>("remove_permission_rule", { ...target, kind, rule });

export const setMcpServer = (target: LayerTarget, name: string, server: McpServerConfig) =>
  invoke<void>("set_mcp_server", { ...target, name, server });

export const removeMcpServer = (target: LayerTarget, name: string) =>
  invoke<void>("remove_mcp_server", { ...target, name });

export const testMcpServer = (server: McpServerConfig, projectRoot: string | null) =>
  invoke<McpTestResult>("test_mcp_server", { server, projectRoot });

/** Every registered feature; only `available` ones are listed in Settings. */
export const featureCatalog = () => invoke<FeatureSpec[]>("feature_catalog");

/** Starts the sidecar when one is configured; `projectRoot` null tests the user settings. */
export const testDecisionModel = (projectRoot: string | null) =>
  invoke<DecisionModelTest>("test_decision_model", { projectRoot });

export const decisionModelStatus = (projectRoot: string | null) =>
  invoke<DecisionModelStatus>("decision_model_status", { projectRoot });

/** Starts or resumes the download in the background; poll the status for progress. */
export const downloadDecisionModel = (projectRoot: string | null) =>
  invoke<DecisionModelStatus>("download_decision_model", { projectRoot });

/** Partial files stay, so downloading again resumes. */
export const cancelDecisionModelDownload = () => invoke<void>("cancel_decision_model_download");

export const removeDecisionModel = (projectRoot: string | null) =>
  invoke<DecisionModelStatus>("remove_decision_model", { projectRoot });

/** Replaces the layer's hooks for `event`; an empty list removes them. */
export const setHooks = (target: LayerTarget, event: string, hooks: HookConfig[]) =>
  invoke<void>("set_hooks", { ...target, event, hooks });

/** Stored keys by bucket: `openrouter`, `opencode`, `anthropic`, `openai`, `github-copilot`, `brave`, `tavily`, `exa`, `custom:<host>`. */
export const credentialStatus = () => invoke<Record<string, KeyStatus>>("credential_status");

/** Stores the key for the bucket of `baseUrl`; null removes it. */
export const saveApiKey = (baseUrl: string, key: string | null) =>
  invoke<void>("save_api_key", { baseUrl, key });

/** Starts GitHub's device flow for the Copilot API at `baseUrl` (GitHub.com or Enterprise). */
export const copilotLoginStart = (baseUrl: string) =>
  invoke<CopilotDeviceCode>("copilot_login_start", { baseUrl });

/** Asks once whether the user approved `deviceCode`; on `done` the token is saved for `baseUrl`'s bucket. */
export const copilotLoginPoll = (baseUrl: string, deviceCode: string) =>
  invoke<CopilotLoginPoll>("copilot_login_poll", { baseUrl, deviceCode });

export const saveSearchKey = (backend: SearchBackend, key: string | null) =>
  invoke<void>("save_search_key", { backend, key });

export const trustStatus = (projectRoot: string) =>
  invoke<TrustStatus>("trust_status", { projectRoot });

export const setTrust = (projectRoot: string, trusted: boolean) =>
  invoke<void>("set_trust", { projectRoot, trusted });

/** `projectRoot` null lists user-level extensions only. */
export const listExtensions = (projectRoot: string | null) =>
  invoke<Extensions>("list_extensions", { projectRoot });

/** Reads an extension file listed by `list_extensions`, so edits keep its exact text. */
export const readExtensionFile = (path: string) => invoke<string>("read_extension_file", { path });

/** `name` is relative to the kind's folder without `.md` (`a:b` for `a/b.md`; the folder name for skills). */
export const writeExtensionFile = (
  projectRoot: string | null,
  kind: ExtensionKind,
  name: string,
  content: string,
) => invoke<string>("write_extension_file", { projectRoot, kind, name, content });

export const deleteExtensionFile = (path: string) => invoke<void>("delete_extension_file", { path });

/** `projectRoot` null lists the user file only. */
export const listInstructionFiles = (projectRoot: string | null) =>
  invoke<InstructionFile[]>("list_instruction_files", { projectRoot });

export const writeInstructionFile = (path: string, content: string) =>
  invoke<void>("write_instruction_file", { path, content });
