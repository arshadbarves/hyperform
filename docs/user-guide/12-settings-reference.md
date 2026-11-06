# Settings reference

HyperForm's settings are TOML files. This page explains the files and how
they combine, how the Settings screens map to them, every key with its type
and default, the environment variables HyperForm reads, and how v1 settings
are imported.

## Settings files and layers

Settings are read in layers, lowest precedence first. A later layer wins.

| Layer | File | Use it for |
|---|---|---|
| Defaults | built in | – |
| **User** | `~/.config/hyperform/settings.toml` (Windows: `%APPDATA%\hyperform\settings.toml`) | Your preferences for every project. |
| **Project** | `<project>/.hyperform/settings.toml` | Settings shared with your team through git. |
| **Personal (local)** | `<project>/.hyperform/settings.local.toml` | Your own overrides for this project. HyperForm adds it to `.hyperform/.gitignore` when it writes the file. |
| **Environment** | `HYPERFORM_MODEL`, `HYPERFORM_BASE_URL`, `HYPERFORM_PROVIDER`, `HYPERFORM_SHELL` | Overriding everything, for example in scripts. |

How values combine:

- Single values (strings, numbers, true/false) are replaced by the later
  layer. Tables such as `[provider.headers]` and `[shell.env]` merge key by
  key.
- These lists **combine** across layers (duplicates removed):
  `permissions.allow`, `permissions.ask`, `permissions.deny`,
  `permissions.additional_directories`, `shell.env_passthrough`,
  `shell.sandbox.extra_writable`.
- Hook lists (`[[hooks.<Event>]]`) and `[[decisions.rules]]` are
  **concatenated**: all run.
- `[[verification.checks]]` merge by `id`: a later check with the same id
  replaces the earlier one.
- `[mcp.servers.<name>]` and `[lsp.servers.<name>]`: a later server with the
  same name replaces the earlier one completely.
- Other lists (such as `model.fallbacks` and `verification.auto_checks`)
  are replaced by the later layer.

Problems never stop the app:

- Out-of-range numbers are clamped and reported.
- Unknown keys (typos) are reported and ignored.
- A file with a wrong value type is **skipped entirely** until you fix it;
  while that file is selected, the Settings page says "This settings file
  is not applied until it is fixed" and why.
- Files larger than 1 MiB are refused.
- In an untrusted project, anything that runs programs or loosens
  permissions comes from your user settings only: `hooks`, `mcp`,
  `verification.checks`, `permissions.mode`, `permissions.allow`,
  `permissions.additional_directories`,
  `permissions.auto_allow_read_only_bash`, and the whole `shell`,
  `provider`, `web`, `lsp` and `decisions` sections. The project's `deny`
  and `ask` rules and `permissions.planning` still apply
  ([workspace trust](03-permissions-and-safety.md#workspace-trust)).

The first line of each file is `schema = 2`; the app manages it.

## Settings screens and files

Open Settings with ⌘, (Ctrl+, on Windows and Linux) or the gear at the
bottom of the sidebar. It opens on the page you used last; links elsewhere
in the app, such as **Connect a model** on the project home, open a given
page.
**Back** or Esc closes it. The navigation groups the pages, and each page
starts with one line saying what it is for:

| Group | Page | Keys |
|---|---|---|
| General | Models | `model.*` |
| | Providers | `provider.*`, `model.main`; keys go to `auth.json` |
| | Appearance | `ui.task_report_view`, `ui.output_style` |
| | Pet | `ui.companion`, `ui.pet.*` |
| Agent | Permissions | `permissions.*` |
| | Memory | `AGENTS.md` files |
| | Verification | `verification.*` |
| | Agents & Commands | files in `agents/`, `commands/`, `skills/`, `rules/`, `output-styles/` (user folder or project `.hyperform/`) |
| Integrations | MCP | `mcp.servers.*` |
| | Hooks | `hooks.*` |
| System | Advanced | `context.*`, `agents.*`, `web.*`, `shell.*`, `lsp.*`, `compat.claude`, workspace trust |
| | Experimental | `experimental.*`; `decisions.*` on the **Decision model** card, shown while a decision feature runs (`work.*` is file-only) |
| | About & Updates | version, updates, file locations |

- **Search settings** at the top of the navigation finds single settings
  by name or by everyday words: "budget" finds **Session cost cap**, "api
  key" finds **Providers and API keys**. Pick a result with ↑/↓ and Enter,
  or click it: its page opens, its group unfolds, and the row is briefly
  highlighted. The command palette finds the same settings.
- Pages that edit settings files show **Saving to** at the top right. Its
  menu offers **User**, **This project** and **Personal (local)**, each with
  a short description, and shows the path of the file it writes. Changes
  apply to open chats immediately. Without an open project only **User** is
  available, and the page says "Open a project to edit its project and
  personal settings."
- Some groups fold to their title and one line until you open them: every
  group on **Advanced** (Context, Agent limits, Web, Shell, Language
  servers, Workspace), **Fallbacks** and **Requests** on **Models**, and
  **Folders** on **Permissions**.
- A setting shows a source badge (**User**, **Project**, **Personal** or
  **Environment**) only when a settings file or environment variable sets
  it; hover the badge for the file. **Reset** removes the value from the
  selected file, and a note says when a higher file overrides it.

> **Warning:** When the app saves a settings file it rewrites it, so
> comments you added by hand are lost.

## Model

`[model]` – which models run. See [Models, providers and cost](11-models-providers-and-cost.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `main` | string | `"anthropic/claude-sonnet-4.5"` | Model of the main agent. Empty uses the default. |
| `fast` | string | unset (= main) | Titles, summaries, WebFetch answers, `explore`/`verify` agents. |
| `review` | string | unset (= main) | The `review` agent. |
| `fallbacks` | list of strings | `[]` | Tried in order after a temporary failure. |
| `effort` | `"low"`, `"medium"`, `"high"`, `"max"` | unset | Reasoning effort for new chats. Unset sends none. |
| `max_output_tokens` | integer | `16384` | Output limit per request, 256–200000. |
| `context_window` | integer | unset (catalog, the active provider's listing first) | Overrides the model's context size. |

`[pricing."<model id>"]` – your own prices in US dollars per million
tokens: `input`, `output` (required), `cache_read`, `cache_write`
(optional; default to `input`). The id matches with or without a `vendor/`
prefix.

## Provider

`[provider]` – how the model API is reached.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `kind` | `"auto"`, `"openai_chat"`, `"anthropic"` | `"auto"` | Wire format. `auto` uses Anthropic for `anthropic.com` hosts, OpenAI-compatible otherwise. |
| `base_url` | string | `"https://openrouter.ai/api/v1"` | API address; trailing `/` is removed. |
| `headers` | table | `{}` | Extra HTTP headers for every model request. |
| `cache_control` | bool | unset | Prompt caching. Unset: on for Anthropic and OpenRouter, off elsewhere. |

## Permissions

`[permissions]` – see [Permissions and safety](03-permissions-and-safety.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `"default"`, `"acceptEdits"`, `"bypass"` | `"default"` | Permission mode new chats start in. Also accepts `bypassPermissions`, `accept_edits`. The older `"plan"` (or `readOnly`) still works: it means `"default"` with `planning = true`. |
| `planning` | bool | `false` | Start new chats in [plan mode](05-plan-mode-questions-and-todos.md). Allowed from an untrusted project, because it only restricts. |
| `allow` | list of rules | `[]` | Run without asking. |
| `ask` | list of rules | `[]` | Always ask. |
| `deny` | list of rules | `[]` | Never allow, in any mode. |
| `additional_directories` | list of paths | `[]` | Extra folders the agent may read and edit. `~` and project-relative paths allowed. |
| `auto_allow_read_only_bash` | bool | `true` | Run recognised read-only commands without asking. |

## Context

`[context]` – see [Memory and context](07-memory-and-context.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `compact_at_percent` | integer | `92` | Summarize older history at this context fill, 50–99. |
| `keep_recent_tool_results` | integer | `8` | Tool results kept word for word when older ones are cleared. |
| `repo_map` | bool | `true` | Include the repository map. |
| `repo_map_chars` | integer | `6000` | Size of the repository map. |

## Agents

`[agents]` – see [Agents](04-agents.md#limits).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `max_concurrent` | integer | `6` | Subagents at once (at least 1). Applies to chats opened afterwards. |
| `max_depth` | integer | `2` | Subagent nesting depth; 0 disables subagents. |
| `max_turns` | integer | `200` | Model turns per agent run (at least 1). |
| `session_cost_cap_usd` | number | `0.0` | Stop a chat at this spend; 0 turns it off. |

## Verification

`[verification]` – see [Verification](10-verification.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `"off"`, `"report"`, `"auto"`, `"strict"` | `"report"` | What happens at the end of a turn that changed files. |
| `max_continuations` | integer | `3` | Times auto/strict may send a turn back, 0–10. |
| `auto_checks` | list | `["test"]` | Check kinds or check ids run by auto/strict. A custom check can be listed as `e2e` or `custom:e2e`. |

`[[verification.checks]]` – one table per check: `id` (required),
`command` (required), `label` (default: the id), `kind` (`test`, `build`,
`lint`, `typecheck`, `format`, `custom`; default `custom`), `cwd` (relative
to the project), `timeout_secs` (default 600, at least 1).

## Hooks

`[[hooks.<Event>]]` – events: `SessionStart`, `UserPromptSubmit`,
`PreToolUse`, `PostToolUse`, `Stop`, `SubagentStop`, `PreCompact`,
`Notification`, `SessionEnd`. Fields: `command` (required), `matcher`
(regular expression, optional), `timeout_secs` (default 60, at least 1).
See [Hooks](08-hooks.md).

## MCP

`[mcp.servers.<name>]` – fields: `command`, `args`, `env`, `cwd` (stdio) or
`url`, `headers` (HTTP); `enabled` (default `true`), `disabled_tools`
(default `[]`), `timeout_secs` (default 60). Exactly one of `command` and
`url`. See [MCP and code intelligence](09-mcp-and-code-intelligence.md).

## LSP

| Key | Type | Default | Meaning |
|---|---|---|---|
| `lsp.enabled` | bool | `true` | Use language servers. |

`[lsp.servers.<name>]` – fields: `command` (required unless the server is
turned off), `args`, `extensions`, `root_markers`, `enabled` (default
`true`; `false` also turns off a built-in server of that name).

## Web

`[web]` – how the agent searches and reads the web.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `search_backend` | `"none"`, `"brave"`, `"tavily"`, `"exa"`, `"searxng"` | `"none"` | Web search provider. With `none` the WebSearch tool isn't offered. Brave, Tavily and Exa need a key (stored from Settings → Advanced → Web, or `BRAVE_API_KEY` / `TAVILY_API_KEY` / `EXA_API_KEY`); without one, search stays off. |
| `search_url` | string | unset | Base URL of your SearXNG instance (required for `searxng`). |
| `fetch_extract` | bool | `true` | WebFetch hands the page to the fast model, which answers the agent's question about it. `false` returns the page itself (as markdown). |
| `allow_private_network` | bool | `false` | Let WebFetch reach localhost and private network addresses. |

## Shell

`[shell]` – how commands run.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `path` | string | unset (detect) | The shell. macOS/Linux: bash, zsh, sh, dash or ksh (default: your login shell if bash or zsh, else `/bin/bash`, then `/bin/sh`). Windows: `bash` (Git Bash), `powershell`/`pwsh`, `cmd`, or a path (default: Git Bash, then PowerShell, then cmd). |
| `env_passthrough` | list of names | `[]` | Extra variables copied from HyperForm's environment into commands, for example `SSH_AUTH_SOCK`. |
| `env` | table | `{}` | Variables set for every command; they override everything else. |

Commands (Bash, background shells, checks, hooks) don't get your whole
environment. They get `PATH`, `HOME`, `SHELL`, `TERM`, `LANG`, `LC_ALL`,
`TMPDIR`, `USER`, `LOGNAME` (plus Windows system variables such as
`USERPROFILE`, `APPDATA` and `Path`), the `env_passthrough` variables, and
`GIT_TERMINAL_PROMPT=0`, `PAGER=cat`, `GIT_PAGER=cat`, `HYPERFORM=1`, then
`env`.

`[shell.sandbox]` – see [the sandbox](03-permissions-and-safety.md#the-sandbox).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | bool | `false` | Run commands in the OS sandbox. |
| `allow_network` | bool | `false` | `false` blocks all connections except localhost. |
| `extra_writable` | list of paths | `[]` | More writable folders (`~/` = home, relative = project). |
| `auto_allow` | bool | `true` | Run sandboxed commands whose writes stay inside without asking. |

## UI and compatibility

| Key | Type | Default | Meaning |
|---|---|---|---|
| `ui.companion` | `"lively"`, `"calm"`, `"off"` | `"lively"` | How lively the [pet](02-everyday-use.md#your-pet) is: **Lively** reacts to the agent and to you (typing, scrolling, stepping away) and roams if `ui.pet.roam` is on, **Calm** reacts only to the agent and stays in the title bar, **Off** shows a small dot instead of the pet. Settings → Pet → Liveliness. |
| `ui.pet.name` | string | `"Zen"` | The pet's name. Trimmed; empty means `"Zen"`; longer than 24 characters is cut with a warning. Settings → Pet → Name. |
| `ui.pet.look` | `"pearl"`, `"mint"`, `"sky"`, `"lilac"`, `"peach"`, `"graphite"` | `"pearl"` | The pet's body color. Settings → Pet → Look. |
| `ui.pet.roam` | bool | `true` | At the **Lively** level, let the pet leave the title bar for the composer, the sidebar and the side panel's tab bar while nothing needs you. Settings → Pet → Let it roam. |
| `ui.output_style` | string | unset | Name of an [output style](07-memory-and-context.md#output-styles). |
| `ui.task_report_view` | `"quiet"`, `"compact"`, `"detailed"` | `"quiet"` | How much the receipt under each turn shows: **Quiet** only a Verified, Unverified or Failed badge; **Compact** adds the changed files, duration and cost; **Detailed** adds tokens, the Not applicable badge and unfolded checks ([details](02-everyday-use.md#the-turn-receipt)). Settings → Appearance → Task report detail. |
| `compat.claude` | bool | `true` | Also read `.claude/` folders (agents, commands, skills) and `CLAUDE.md` files. `.claude/settings.json` is never read. |

```toml
[ui]
companion = "lively"

[ui.pet]
name = "Pip"
look = "lilac"
roam = false      # stay in the title bar (and on the project home)
```

The pet's level, streak and accessory are not settings: they are kept in
`pet.json` in the data folder ([Sessions and data](13-sessions-and-data.md#where-things-live)).

## Experimental

`[experimental]` – one line per feature id, set to `"off"` (default),
`"shadow"` or `"on"` (`false` and `true` also work). Settings →
Experimental writes them. This version has 23 ids: the 21 listed in
[Decision features](16-decision-features.md#all-features-at-a-glance),
`work_plans` ([work plans](18-work-plans.md)) and `work_parallel`
([parallel tasks](18-work-plans.md#parallel-tasks)), both Off or On only.
`work_parallel` needs `work_plans`; without it, it stays off with a
warning.
A stable feature is always on and its line is ignored with a warning;
`shadow` or `on` for a feature this version doesn't include stays off with
a warning; `shadow` for a feature without shadow support stays off with a
warning; an unknown id is reported as an unknown key. See
[Experimental features](15-experimental-features.md).

## Work

`[work]` – limits for [work plans](18-work-plans.md),
used only while `work_plans` is on; the last three matter only with
`work_parallel` on too. They have no Settings screen; set them in a
settings file.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `packet_tokens` | integer | `4000` | Estimated tokens one task packet may take, 500–32,000. The goal, invariants and the task with its criteria always go in; dependencies, evidence and notebook entries fill the rest. |
| `sensitive_paths` | list of globs | `[]` | Paths relative to the project, such as `"src/auth/**"`. A plan with a task whose scope touches one needs deep planning (judged when the agent sends the plan and when you approve it), so the [audit agent](18-work-plans.md#the-plan-audit) reviews it; the audit also expects a test criterion for tasks that touch one. |
| `max_attempts` | integer | `3` | Times an agent may start one task, 1–20. After that it must mark the task failed or ask you; your **Retry** gives it a fresh set. |
| `max_parallel` | integer | `3` | Tasks that may run at once in parallel helpers, 1–16; never more than `agents.max_concurrent`. |
| `merge` | `"ask"` or `"auto"` | `"ask"` | How a parallel helper's change comes back: `"ask"` waits for you to apply it; `"auto"` applies it at once when it merges cleanly (a conflict still stops and asks). A project's `"auto"` counts only once you trust it. |
| `serial_paths` | list of globs | lockfiles, manifests, generated output | A task whose scope names one never runs beside others. |

The default `serial_paths` are `**/Cargo.lock`, `**/Cargo.toml`,
`**/package-lock.json`, `**/npm-shrinkwrap.json`, `**/yarn.lock`,
`**/pnpm-lock.yaml`, `**/bun.lock`, `**/bun.lockb`, `**/package.json`,
`**/poetry.lock`, `**/uv.lock`, `**/Pipfile.lock`, `**/pyproject.toml`,
`**/go.mod`, `**/go.sum`, `**/Gemfile`, `**/Gemfile.lock`,
`**/composer.json`, `**/composer.lock`, `**/generated/**`,
`**/__generated__/**` and `crates/hyperform-gui/ui/src/lib/protocol/**`. A
list you set replaces them, so repeat the ones you want to keep; an
untrusted project can only add to yours.

A value out of range is clamped with a warning such as "work.max_attempts
= 50 is out of range; using 20"; blank globs are dropped with a warning.
Another `merge` value makes HyperForm skip that settings file, with a warning.

```toml
[work]
packet_tokens = 6000
max_attempts = 2
max_parallel = 2
merge = "auto"
```

## Decisions

`[decisions]` – the [decision model](15-experimental-features.md#the-decision-model)
that the decision features ask: its runtime (`"sidecar"` or `"native"`),
endpoint, checkpoint, timeout, confidence threshold and calibration, plus
`[decisions.prefetch]`, `[decisions.routing]`, `[decisions.loop_guard]`,
`[decisions.task_view]` and your own `[[decisions.rules]]`, which add up
across layers like hooks. Every key, with its default and range, is in the
[decision settings reference](17-decision-settings.md).

## Environment variables

| Variable | Effect |
|---|---|
| `HYPERFORM_MODEL` | Overrides `model.main`. |
| `HYPERFORM_BASE_URL` | Overrides `provider.base_url`. |
| `HYPERFORM_PROVIDER` | Overrides `provider.kind` (`auto`, `openai_chat`, `anthropic`). |
| `HYPERFORM_SHELL` | Overrides `shell.path`. |
| `HYPERFORM_API_KEY` | API key for the configured provider (wins over all others). |
| `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENCODE_API_KEY` | API key for that provider (wins over a stored key). GitHub Copilot has no variable of its own. |
| `BRAVE_API_KEY`, `TAVILY_API_KEY`, `EXA_API_KEY` | Web search keys. |
| `HYPERFORM_CONFIG_DIR` | Use another config folder (settings, keys, trust, user extensions). |
| `HYPERFORM_DATA_DIR` | Use another data folder (chats, checkpoints, cache, log). |
| `HYPERFORM_GIT_BASH_PATH` | Windows: the Git Bash `bash.exe` to use. |
| `RUST_LOG` | Detail of the app log, for example `debug` (default `info`). |

HyperForm sets `HYPERFORM=1` for commands it runs, `HYPERFORM_PROJECT_DIR`
and `CLAUDE_PROJECT_DIR` for hooks, and the `LAYA_*` variables above for
the decision model's sidecar. `decisions.api_key_env` names a variable of
your choice.

## Settings files initialization

When HyperForm AI starts for the first time:

- **User settings:** if `settings.toml` does not exist yet, a commented
  default `settings.toml` file is created with `schema = 2`.
- **Project settings:** personal overrides live in
  `<project>/.hyperform/settings.local.toml` (git-ignored), while shared team
  settings live in `<project>/.hyperform/settings.toml`.

API keys are stored securely per-host in `auth.json` next to your user settings,
never in the settings file itself.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Sessions and data](13-sessions-and-data.md) · [Troubleshooting](14-troubleshooting.md) · [Experimental features](15-experimental-features.md) · [Decision settings reference](17-decision-settings.md)
