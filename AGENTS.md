# AGENTS.md — Structure Contract (read before writing code)

This repository is an AI coding-agent harness. **Any agent or human
modifying this codebase MUST maintain the structure defined below.** The
structure exists so every file stays small, single-purpose, and easy to
navigate. Violations are review-blocking.

Companion documents: [Engineering & Coding Style Guide](docs/engineering/style-guide.md),
[v2 engine architecture](docs/architecture/v2-engine.md),
[GUI UI guide](docs/design/gui-ui-guide.md), [status](docs/status.md), and
the [documentation contract](docs/AGENTS.md) (with the
[user guide](docs/user-guide/README.md) and
[how it works](docs/how-it-works/README.md) it keeps current).
Update this contract in the same change as any crate or frontend
restructuring.

The desktop GUI is the only product frontend. Do not add a terminal or
headless replacement. The agent's shell tool and private integration-test
fixtures remain supported; neither is a public command-line product.

## Crates

```
crates/
├── z-engine-protocol/     # leaf: ids, conversation model, Event/Command (ts-rs -> ui/src/lib/protocol/)
├── z-engine-prompts/      # leaf: ALL prompt prose as markdown under prompts/<area>/*.md
├── z-engine-llm/          # ModelClient seam, openai_chat + anthropic + responses adapters, GitHub Copilot client, retry, fallback, catalog, cost
├── z-engine-config/       # settings layering + v1 migration, credentials, trust, extension discovery, feature registry
├── z-engine-policy/       # pure permission engine: rules, modes, shell analysis
├── z-engine-host/         # the ONLY OS/network adapter: fs, processes, jobs, search, git, checkpoints, web, model download
├── z-engine-integrations/ # MCP (stdio + HTTP) and LSP clients over one JSON-RPC core
├── z-engine-context/      # pure prompt assembly, reminders, repo map, tokens, compaction planning
├── z-engine-verify/       # check discovery, records, output parsing, freshness, outcome
├── z-engine-store/        # session logs, subagent transcripts, plan logs, artifacts, index, v1 import, decision dataset
├── z-engine-decisions/    # decision layer: questions, answers, DecisionProvider (rules, systemone, onnx, hybrid), native model pins, cache, calibration, trace
├── z-engine-work/         # pure work-plan kernel: task graph, state machines, plan-log fold, authority and completion gate, packets, reconciliation and replan holds, dispatch and parallel eligibility, interrupted tasks (resume), needs you, audit, health
├── z-engine-tools/        # Tool trait, capability ports, registry, builtin/<tool>.rs (one file per tool)
├── z-engine-engine/       # orchestrator: sessions, agent runs, gating, hooks, jobs, commands, decision seams and uses, GUI queries
├── z-engine-testkit/      # dev-only: ScriptedModel, FixtureRepo, EventRecorder
└── z-engine-gui/
    ├── src-tauri/         # Tauri shell: builder wiring, AppState, event bridge, commands/<domain>.rs
    └── ui/                # Svelte 5 frontend
website/                   # VitePress docs site (config, theme, sidebar); the content stays in docs/
```

Dependency rules (arrows mean "may import"):

```
z-engine-gui -> engine, protocol, config
engine       -> every crate below
tools        -> host, policy
integrations -> host
verify       -> host
context      -> (leaf crates only)
llm, config, policy, host, store, decisions, work -> (leaf crates only)
every crate  -> protocol, prompts
testkit      -> llm (dev-dependency of other crates only)
```

- Only `engine` knows concrete implementations; tools reach engine
  services through capability traits in `z-engine-tools::ports`. The GUI
  shell calls `Engine` (sessions, settings, catalog, and the queries in
  `engine/queries/`) and uses `config` only for settings files, credentials,
  trust and extension/instruction discovery.
- Only `host` touches the OS or network (processes, git, HTTP). The
  documented exceptions: `integrations` owns its server processes and MCP
HTTP connections, `llm` sends the model-provider requests (including
 GitHub Copilot's device sign-in and `/models`) and the models.dev
 requests, `decisions` sends `POST /v1/systemone` to the decision model
  (loopback unless `decisions.allow_remote`; its optional sidecar is
  started by `engine` through `host` background shells) and, with the
  `onnx` feature, reads the native model files that `host` downloaded,
  and the GUI shell checks GitHub for updates and installs them,
  and opens or reveals project files (`open_path`, `reveal_path`) through
  the opener plugin. `config` and `store` read/write their own files;
  `context`, `policy` and `work` do no I/O.
- Prompt prose lives only in `crates/z-engine-prompts/prompts/<area>/`,
  one `pub const` per file in `src/<area>.rs`. Tool descriptions are
  `prompts/tools/<snake_name>.md` (e.g. `multi_edit.md`). Never inline
  prompt text in logic files.
- Protocol types are the GUI contract: change them only in
  `z-engine-protocol` (config types in `z-engine-config`), then run
  `cargo test -p z-engine-protocol` / `-p z-engine-config` and commit the
  regenerated `ui/src/lib/protocol/**/*.ts` in the same change.
- Tool names and input schemas follow Claude Code (`Read`, `Edit`, `Bash`,
  `Grep`, `TodoWrite`, `Agent`, ...). Custom agents, commands and skills are
  markdown with YAML frontmatter; `.claude/` folders are read for
  compatibility.

## Golden rules

1. **File budget:** target ≤300 lines; hard cap 400. When a file would
   exceed the cap, split it by responsibility — never by percentage.
2. **One file = one reason to change** (SRP). A file named after a thing
   contains only that thing.
3. **`mod.rs` / `lib.rs` / `main.rs` are composition roots only**: module
   declarations + re-exports (the GUI `main.rs`: builder wiring, <160
   lines). No logic beyond ~30 lines of glue.
4. **Prompts are data, not code** (see above).
5. **Dependency direction** as above; cross-crate calls go through public
   traits and types, never concrete internals.
6. **Errors:** library crates use typed `thiserror` enums. The GUI shell may
   use `anyhow`; its commands return display strings to the webview.
7. **Tests live next to what they test** (`#[cfg(test)] mod tests`) or in
   `tests/` for integration flows. One concern per integration file.
8. **Every new feature starts Experimental**: a `FeatureId` in
   `z-engine-config/src/features/registry.rs` (owner, graduation criteria,
   default off), checked only through `Settings::feature(id)`. It becomes
   Stable once it meets its criteria (recorded in `docs/status.md`); a flag
   never disables a safety invariant, and a decision use may only tighten
   (Allow to Ask), never loosen.

## GUI shell (`crates/z-engine-gui/src-tauri/src`)

```
main.rs          # builder wiring: runtime, logging, plugins, handler list, setup, shutdown
state.rs         # AppState { engine, workspaces, pet store, active project }
events.rs        # EventSink -> Tauri event `engineEvent` (payload: EventEnvelope)
window.rs        # main window, title bar, vibrancy/Mica
logging.rs       # <data dir>/z-engine-gui.log
workspaces.rs    # <data dir>/workspaces.json registry
pet.rs           # <data dir>/pet.json: the pet's growth (temp-file write, 64 KB cap)
layers.rs        # settings scope -> layer file, layer tables (incl. in-memory v1 import)
guard.rs         # path validation: extension and instruction files, files opened inside a project
ipc.rs           # IpcResult, error text, JSON for engine query results
commands/        # ALL #[tauri::command] fns, one file per domain:
                 # session, catalog, workspace, settings, access (keys, trust),
                 # extensions, app, update, pet, work (plans, packets, why)
```

## Frontend (`crates/z-engine-gui/ui/src`)

**Svelte 5 + Bits UI + Vite**, with Three.js for the pet only. Canonical UI rules:
[`docs/design/gui-ui-guide.md`](docs/design/gui-ui-guide.md).

```
ui/src/
├── App.svelte                  # composition root (wiring only)
├── styles/                     # EVERY stylesheet: tokens, base, motion, materials, kit + one or more per area
├── lib/protocol/               # GENERATED by ts-rs (protocol + config/); never edit by hand
├── lib/commands/*.ts           # ONLY Tauri invoke wrappers: engine, workspace, app, settings, pet, work
├── lib/runtime/                # event listening (listen.ts), session, project, project plans and inbox stores, catalogs, the pet's growth, actions, plan flow; dev/ (dev builds only: the mock backend for browser QA)
├── lib/domain/                 # pure helpers, tested with vitest:
│   ├── sessionView/            #   EventEnvelope -> session view reducer
│   ├── timeline/               #   turns, blocks and tool-run groups for the transcript
│   ├── tools/                  #   per-tool presentation helpers
│   ├── settings/               #   forms, scopes, provenance, credentials, settings search
│   ├── work/                   #   work plans: panel state, task tree, status, progress, criteria, notebook, changes, why, trace, packet, needs you, now lanes, health, results, parallel, state banner, holds, resume, interrupted helpers, plan picker, home card, inbox items
│   └── pet/                    #   the pet: pose, emotions, motion, looks, growth, perches, behavior; face shapes, 3D rig, portrait
├── lib/pet3d/                  # the pet in 3D (Three.js): shared WebGL renderer, frame loop, palette, scene (ONLY three import)
├── lib/stores/                 # composer, settings, feature catalog, UI chrome (side panel, island, live status, per-plan view state), the pet's UI state, shortcuts, confirm, onboarding, plan actions
├── lib/ui/                     # Bits UI wrappers + Icon + Button + small kit (Pill, SearchField, SelectionCapsule), springs, motion, presence, perch, whenVisible (ONLY bits-ui import)
└── components/
    ├── chat/ chat/tools/       # transcript, turn summaries and actions, composer, approval and tool cards, tool-run groups, route chip, task-view divider
    ├── planning/               # questions, the Plan tab (document, work plan tree, task detail and result, plan picker, state banner, needs-you strip, now lanes, findings, notebook, change set), todos, decision suggestion cards
    ├── agents/                 # the Agents tab (helpers and jobs), apply cards, subagent transcripts
    ├── settings/               # settings page, grouped nav, scope menu and tabs, Experimental page and decision model card
    ├── sidepanel/              # the side panel: tab band, Changes / Plan / Agents / Context
    ├── overlays/               # Changes tab, prompt inspector (Context tab, with its Decisions and Task packet sections), palette, worktree dialog, shell drawer
    ├── chrome/                 # AppShell, MainStage, title bar with the island and satellites, full-window page frame, splash
    ├── pet/                    # the pet: 3D or flat (SVG) view, island slot and portrait, roaming layer, card
    ├── sidebar/                # projects and chats, nav, footer
    ├── home/                   # project home: starters, Continue / Changes / setup cards
    ├── inbox/                  # Activity inbox: needs you, finished, notices
    └── onboarding/             # first-run steps
```

Rules: screens never `invoke()` or import `bits-ui`; only `lib/pet3d/`
imports `three`, and it is loaded lazily (`components/pet/petView.ts`);
engine events are listened to only in `lib/runtime/listen.ts` (the
updater's progress events in `lib/updateStore.ts`); stylesheets live only
in `styles/`; file budget ≤300 / hard cap 400. Do not add SvelteKit,
Tailwind, shadcn-svelte, React, or a second design system.

## How to add things (follow exactly)

| Adding… | Do this |
|---|---|
| a tool | `z-engine-tools/src/builtin/<snake_name>.rs` implementing `Tool`, declared in `builtin/mod.rs` and registered in `builtin/list.rs`; its name in `z-engine-tools/src/names.rs`; description in `z-engine-prompts/prompts/tools/<snake_name>.md` with a `pub const` and a table entry in `src/tools.rs` |
| a prompt | `z-engine-prompts/prompts/<area>/<name>.md` + one `pub const` in `src/<area>.rs` |
| an IPC command | fn in the matching `src-tauri/src/commands/<domain>.rs` with `#[tauri::command]`, add it to `generate_handler!` in `main.rs`, wrapper in `ui/src/lib/commands/<domain>.ts` |
| a GUI query | `impl Engine` method in `z-engine-engine/src/engine/queries/<topic>.rs` with camelCase `Serialize` results and unit tests |
| a GUI screen | `ui/src/components/<area>/<Name>.svelte`; use `lib/ui` primitives; follow the UI guide |
| a GUI primitive | wrapper in `ui/src/lib/ui/` around Bits UI; never import `bits-ui` from a screen |
| a config key | field in `z-engine-config/src/settings/<section>.rs` (+ default), commented example in `default_config.toml`, v1 mapping in `migrate/convert.rs` if v1 had it; regenerate TS |
| an event/command variant | `z-engine-protocol` enum, handle it in the engine (actor / emitter) and in `lib/domain/sessionView`; run `cargo test -p z-engine-protocol` and commit the TS |
| a built-in agent | `z-engine-prompts/prompts/agents/<name>.md` (frontmatter) + `BUILTIN` entry in `src/agents.rs` |
| a hook event | `HOOK_EVENTS` in `z-engine-config/src/settings/hooks.rs`, fire it from `z-engine-engine/src/hooks/`, document it in `docs/architecture/v2-engine.md` and `docs/user-guide/08-hooks.md` |
| a feature | a `FeatureId` variant (`features/id.rs`) and its `FeatureSpec` in `z-engine-config/src/features/registry.rs` (Experimental, owner, graduation criteria, `available` once built); gate the code on `settings.feature(id)`; it is listed in Settings > Experimental by itself; document it in `docs/user-guide/15-experimental-features.md` (a decision feature in `16-decision-features.md`, its settings in `17-decision-settings.md`; a work-plan feature in `18-work-plans.md`) and the CHANGELOG as Added (experimental); regenerate TS |
| a decision use | `z-engine-engine/src/decisions/uses/<name>.rs` implementing `DecisionUse` for its seams, added to `USES` in `decisions/registry.rs`; question wording in `z-engine-prompts/prompts/decisions/<name>.md` with a `pub const` in `src/decisions.rs`; every failure must behave as Off (extend `decisions/seams/tests.rs`) |

Every row above also means updating the documentation (next section).

## Documentation (keep it in sync)

Documentation is part of the change, not a follow-up. Any change that adds,
changes or removes something a user or contributor can observe (a UI
surface, command, tool, agent, hook, setting or default, permission
behavior, data location, protocol event, crate or crate responsibility)
updates the affected pages in the same change:

- follow [docs/AGENTS.md](docs/AGENTS.md): it maps each kind of change to
  the pages to update and says where each fact is defined in the code;
- keep the [user guide](docs/user-guide/README.md) (how to use the app) and
  [how it works](docs/how-it-works/README.md) (plain words, mechanism,
  developer detail) true to the code; never document unshipped behavior;
- add an entry under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) for
  user-visible changes;
- delegate the update to the `docs-maintainer` subagent
  ([.claude/agents/docs-maintainer.md](.claude/agents/docs-maintainer.md))
  when your tool supports subagents; otherwise do it yourself;
- say in your final summary which documents you updated, or why none
  needed to change.

## Before you commit

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings   # 0 warnings required
cargo test --workspace                                   # all green required
npm test --prefix crates/z-engine-gui/ui && npm run check --prefix crates/z-engine-gui/ui
python3 scripts/check_docs_links.py                     # docs links resolve
wc -l $(git diff --name-only | grep '\.rs$')             # respect the 400 cap
```

If your change pushes any file past 400 lines, split it first. If you
find an existing violation, fix the part you touch; do not grow it.
