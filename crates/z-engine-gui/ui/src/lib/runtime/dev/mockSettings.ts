import type { FeatureSpec } from "../../protocol/config/FeatureSpec";
import type { Settings } from "../../protocol/config/Settings";

/**
 * The mock backend's settings (lib/runtime/dev/mockBackend.ts): the engine's
 * defaults with work plans and parallel helpers on, so the Plan tab, the
 * home Continue card, the Inbox and the plan picker all show.
 */
export const MOCK_SETTINGS: Settings = {
  model: {
    main: "anthropic/claude-sonnet-4.5",
    fast: null,
    review: null,
    fallbacks: [],
    effort: null,
    max_output_tokens: 16384,
    context_window: null,
  },
  provider: { kind: "auto", base_url: "https://openrouter.ai/api/v1", headers: {}, cache_control: null },
  permissions: {
    mode: "default",
    planning: false,
    allow: [],
    ask: [],
    deny: [],
    additional_directories: [],
    auto_allow_read_only_bash: true,
  },
  context: { compact_at_percent: 92, keep_recent_tool_results: 8, repo_map: true, repo_map_chars: 6000 },
  agents: { max_concurrent: 6, max_depth: 2, max_turns: 200, session_cost_cap_usd: 0 },
  verification: { mode: "report", max_continuations: 3, auto_checks: ["test"], checks: [] },
  hooks: {},
  mcp: { servers: {} },
  lsp: { enabled: true, servers: {} },
  web: { search_backend: "none", search_url: null, fetch_extract: true, allow_private_network: false },
  shell: {
    path: null,
    env_passthrough: [],
    env: {},
    sandbox: { enabled: false, allow_network: false, extra_writable: [], auto_allow: true },
  },
  ui: {
    task_report_view: "quiet",
    output_style: null,
    companion: "lively",
    pet: { name: "Zen", look: "pearl", roam: true },
  },
  compat: { claude: true },
  pricing: {},
  experimental: { work_plans: "on", work_parallel: "on" },
  decisions: {
    endpoint: "http://127.0.0.1:8000",
    api_key_env: null,
    checkpoint: "multilingual",
    max_len: 1024,
    timeout_ms: 250,
    threshold: 0.8,
    max_batch: 16,
    allow_remote: false,
    runtime: "sidecar",
    sidecar: { command: null },
    record_dataset: false,
    calibration: {},
    prefetch: { max_files: 3, max_tokens: 6000 },
    routing: { allow_model_switch: false },
    loop_guard: { max_reminders: 3 },
    task_view: { keep_recent_exchanges: 2 },
    rules: [],
  },
  work: {
    packet_tokens: 4000,
    sensitive_paths: [],
    max_attempts: 3,
    max_parallel: 3,
    merge: "ask",
    serial_paths: ["**/Cargo.lock", "**/package-lock.json"],
  },
};

function feature(id: FeatureSpec["id"], title: string, summary: string, requires: FeatureSpec["id"] | null): FeatureSpec {
  return {
    id,
    title,
    summary,
    stage: "experimental",
    group: "work",
    supportsShadow: false,
    available: true,
    requires,
    owner: "mock",
    graduationCriteria: "Mock backend only.",
  };
}

export const MOCK_FEATURES: FeatureSpec[] = [
  feature("work_plans", "Work plans", "A plan of tasks the agent builds and keeps.", null),
  feature("work_parallel", "Parallel helpers", "Independent tasks run at once in worktrees.", "work_plans"),
];
