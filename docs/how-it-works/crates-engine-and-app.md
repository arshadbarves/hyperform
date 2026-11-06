# The engine, app and test crates

The three crates that put the others together: the engine that conducts
every session, the desktop app around it, and the kit that tests the engine
without a real provider. How all the crates fit together, and the crates
they build on, are in [The crates](crates.md). Terms: the
[glossary](glossary.md). Part of [How HyperForm works](README.md).

## hyperform-engine

**In plain words.** The conductor: it runs each session, asks the model,
sends every tool call through the gate, keeps helpers and background jobs
going, saves everything, and announces each change as an event.

**How it works**
- Owns: the public `Engine` API and GUI queries; a session actor and event
  emitter per open session; agent runs (round loop, context pressure,
  compaction, stop boundary); the tool gate (hooks, policy, batched
  approvals, ordered execution); the broker for approvals, questions and
  plans; subagents, worktrees and jobs; hooks; slash commands; per-session
  MCP and language servers; verification modes; the decision layer's
  service, seams, uses, sidecar and native runtime; work plans (the shared
  plan handles, packets, evidence and reconciliation; task-bound helpers,
  their reports, merging their changes back and verifying them, the
  coordinator's reminders, scope drift at the gate, the audit agent,
  stopping helpers on a pause, hold or move, resuming a plan when a chat
  opens or attaches it, and the chat that drives each plan).
- It alone knows the concrete implementations and wires them into tools
  through ports.
- Must not: depend on the GUI, inline prompt prose, or reach the OS other
  than through host and integrations.

**For developers**
- [`src/`](../../crates/hyperform-engine/src/): `engine/` (API, catalog,
  export, and [`queries/`](../../crates/hyperform-engine/src/engine/queries/):
  diffs, git summary and worktrees, context breakdown, catalogs, file
  search, MCP tests, trust, decisions, decision model, work), `session/`, `run/`, `batch/`,
  `broker/`, `orchestration/` (with `task_run.rs` for a plan task's
  helper), `ports/`, `hooks/`, `commands/`, `mcp/`,
  `lsp/`, `verify/`, `settings/`, [`decisions/`](../../crates/hyperform-engine/src/decisions/),
  [`work/`](../../crates/hyperform-engine/src/work/mod.rs) (with
  `dispatch.rs`, `helper.rs`, `merged.rs`, `coordinate.rs`, `drift.rs`,
  `audit.rs`, `findings.rs`, `bound.rs`, `interrupt.rs`, `kept.rs`,
  `driver.rs`, `checkpoint.rs`, `resume.rs`).
- Uses every crate on [The crates](crates.md) (testkit only in tests); used by: gui. Contract: [v2 engine architecture](../architecture/v2-engine.md)
  and, for work plans, [v2-engine-work.md](../architecture/v2-engine-work.md).

## hyperform-testkit

**In plain words.** The rehearsal kit: a pretend model that follows a
script, throwaway project folders and an event recorder, so tests run the
real engine without a real provider.

**How it works**
- Owns: `ScriptedModel` (a deterministic `ModelClient`), `FixtureRepo`
  (temporary projects, optionally git repositories) and `EventRecorder`
  (collects events and waits for conditions).
- Must not: ship in the product; it is only ever a dev-dependency.

**For developers**
- [`src/`](../../crates/hyperform-testkit/src/): `model.rs`, `repo.rs`,
  `events.rs`. Uses protocol and llm; used by: engine tests.

## hyperform-gui

**In plain words.** The storefront: the Svelte window is the shop floor you
see, and the Rust shell behind it is the counter that passes your orders to
the engine.

**How it works**
- Shell (`src-tauri`): builder wiring (one tokio runtime, plugins, handler
  list), `AppState` (engine, workspaces, pet store, active project), the
  `engineEvent` bridge, the window (dark vibrancy on macOS, dark Mica on
  Windows 11, else solid), the log and the pet's growth
  (`<data dir>/hyperform.log`, `pet.json`), `#[tauri::command]`s.
- Frontend (`ui`): Svelte 5, Bits UI, Vite and, for the pet, Three.js;
  generated protocol types, invoke wrappers, one event listener, pure
  reducers, stores, primitives, screens, and every stylesheet in `styles/`.
- Must not: be imported by any crate. The shell uses only engine, protocol
  and config (settings files, credentials, trust, discovery); screens never
  call `invoke()` or import `bits-ui`; no terminal or headless replacement.

**For developers**
- Shell: [`src-tauri/src/`](../../crates/hyperform-gui/src-tauri/src/):
  `main.rs`, `state.rs`, `events.rs`, `window.rs`, `workspaces.rs`,
  `pet.rs`, `layers.rs`, `guard.rs`, `ipc.rs`, `commands/` (one file per
  domain, `work.rs` for work plans).
- Frontend: [`ui/src/`](../../crates/hyperform-gui/ui/src/): `lib/protocol/`,
  `lib/commands/`, `lib/runtime/`, `lib/domain/`, `lib/stores/`, `lib/ui/`,
  `lib/pet3d/`, `styles/`, `components/`. How it works: [the desktop app](features-desktop-app.md),
  [the pet](features-desktop-pet.md); rules: [GUI UI guide](../design/gui-ui-guide.md).

Which files to touch for a common change: [Where do I change X?](where-to-change.md).

See also: [The crates](crates.md) · [The desktop app](features-desktop-app.md) ·
[v2 engine architecture](../architecture/v2-engine.md) · [AGENTS.md](../../AGENTS.md)
