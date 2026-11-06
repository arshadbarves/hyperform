# HyperForm AI Engineering & Coding Style Guide

Status: engineering conventions for new and changed code.
The [structure contract](../../AGENTS.md) takes precedence for the current
layout; the [GUI guide](../design/gui-ui-guide.md) governs visual and component
patterns; the [engine architecture](../architecture/v2-engine.md) describes the
runtime these conventions protect.

## 1. Engineering principles

- The model proposes; deterministic code enforces authority, state transitions,
  execution constraints, and evidence requirements.
- Ship vertical capabilities, not unused frameworks. Every new seam needs a
  real caller, a tested invariant, and one clear owner.
- Preserve user work. Neither cancellation, recovery, nor rollback grants
  permission to discard unrelated repository changes.
- Distinguish successful execution, verified behavior, and task completion.
  A tool returning text or a model ending its response proves neither of the
  latter two.
- Prefer explicit limitations over optimistic fallbacks. Unsupported,
  unavailable, failed, cancelled, and successful are different outcomes.

## 2. Files, modules, and naming

- Target at most 300 lines per file; 400 is the hard cap. Split by
  responsibility, including tests where necessary, never into numbered chunks.
- A file has one reason to change. Avoid `utils`, `helpers`, `common`, or
  `manager` as containers for unrelated behavior.
- Rust files/modules/functions use `snake_case`; types/traits use `PascalCase`;
  constants use `SCREAMING_SNAKE_CASE`; crates use `z-engine-<responsibility>`.
- Svelte components use `PascalCase.svelte`. Follow the surrounding TypeScript
  domain/runtime naming and sibling `*.test.ts` convention.
- `lib.rs`, `mod.rs`, and frontend index files compose modules and re-export
  APIs; at most about 30 lines of wiring, not business logic.
- GUI [main.rs](../../crates/z-engine-gui/src-tauri/src/main.rs) stays below
  160 lines and owns builder wiring, not application behavior.
- Existing violations are not precedent. Extract the responsibility being
  changed before extending it; do not turn a feature into a repository-wide
  cleanup.

## 3. Crate boundaries and abstraction

The dependency rules are in the [structure contract](../../AGENTS.md): the GUI
depends on the engine, protocol and config; only the engine knows concrete
implementations; tools reach engine services through capability ports; only
`z-engine-host` touches the OS or network; `z-engine-context` and
`z-engine-policy` perform no I/O; check discovery never executes commands.
The desktop GUI is the only product frontend: do not add a terminal or headless replacement.
The agent's shell tool and private Rust integration-test fixtures are not
public frontends and remain supported.

Create a module when a cohesive responsibility needs a name. Extract a crate
only when it has an independently testable contract, deliberate dependencies,
and a concrete benefit such as replacing an adapter or isolating compilation.
Do not create empty crates for every box in a future diagram.

- Use `pub(crate)` or private visibility by default. Export the smallest useful
  capability, not internal mutable state.
- Put an interface at the boundary its consumer needs. Keep shared contract
  types dependency-light; do not create an all-purpose shared crate.
- Use concrete types for local implementation. Use traits for interchangeable
  I/O, policy, or testing boundaries, not for every struct.
- Prefer generics for local static dispatch and `Arc<dyn Trait + Send + Sync>`
  where runtime selection or shared lifetime actually requires it.
- Inject providers, executors, clocks, stores, and capability registries at the
  application composition root. Avoid process-global service locators.
- Runtime plugins are initially statically linked registrations, not a dynamic
  library ABI or a new workflow language.
- A crate migration must update manifests, imports, tests, documentation, and
  the structure contract together. Any temporary facade needs a removal gate.

## 4. Ownership and state

The runtime owns authoritative task state. GUI stores are projections plus
ephemeral presentation state, never a second completion authority.

- Use distinct IDs for session, task, attempt, operation, question, evidence,
  and checkpoint when those concepts are introduced.
- Prefer enums for mutually exclusive states over combinations of booleans.
  Validate transitions through one owner.
- Scope mutable capabilities to a task/workspace. Use `Arc` only for genuinely
  shared lifetimes and locks only for genuinely shared mutation.
- Hold synchronous locks briefly; never across `.await`. Poisoning or failed
  state access is an error, not an empty successful result.
- Serialize mutations of the same workspace. A background agent or MCP
  adapter must not obtain a separate, uncoordinated write path.
- Derived caches are replaceable; goals, accepted decisions, operation
  outcomes, and completion evidence are not caches.
- Do not persist a live future, process handle, or model's hidden reasoning.
  Persist explicit steps, concise decisions, inputs, outputs, and evidence.

## 5. Errors and outcomes

Libraries use typed `thiserror` errors with source errors and useful operation
context. Application shells may use `anyhow` at their boundary.

- Distinguish invalid input, denied authority, stale state, unsupported
  capability, I/O/provider failure, timeout, cancellation, and unknown outcome.
- Preserve causes until rendering user/model-facing messages at a boundary.
  Do not flatten new library APIs into `Result<T, String>`.
- No `unwrap`/`expect` for external input, filesystem, transport, or persisted
  state. Invariant-only use requires a locally evident proof; tests may unwrap.
- Do not swallow failures with `let _ =`, broad catches, or success-shaped
  defaults. Best-effort cleanup must retain the original failure and report
  cleanup failures through the established logging/error path.
- A subprocess exit code, parsed diagnostics, and output text are separate
  facts. Empty diagnostics cannot override a failing exit status.
- A timeout after a side effect may mean `outcome_unknown`, not "nothing
  happened." Inspect state before retrying.
- Expected tool failures become structured model-visible results so the agent
  can recover. Runtime integrity or persistence failures block unsafe progress.

## 6. Async work and process lifetimes

- Use Tokio for async I/O. Move blocking filesystem scans, compiler calls, or
  CPU-heavy work off the async executor when they cannot be made async.
- Every spawned task/process has an owner, cancellation behavior, deadline,
  and join/reap path. Do not detach work accidentally.
- Keep Stop (cancel quickly) separate from Pause (finish the current safe unit,
  persist, then acknowledge suspension).
- Use bounded queues or explicit backpressure for new event streams.
  Coalescing display deltas must never drop authoritative operation results.
- Retry only classified transient failures, within a recorded budget. Side
  effects require reconciliation or idempotency before replay.
- Cancellation of a waiter does not prove cancellation of its subprocess or
  remote operation. Persist uncertainty and show it to the user.
- Parallelize independent reads only with declared capabilities; serialize
  writes and unknown external effects.

## 7. Tools, policy, and repository changes

Extend the [Tool](../../crates/z-engine-tools/src/tool.rs) contract with one
file per tool under `crates/z-engine-tools/src/builtin/`. Built-in and MCP
calls share the same execution gate (hooks, policy, approval).

- Validate typed inputs after JSON decoding; reject malformed requests with
  actionable errors. Tool names alone are not an effects model.
- Describe effect scope, required permission, concurrency, timeout, and
  cancellation/replay safety as tool contracts when introduced.
- Resolve and check targets against the workspace before granting access.
  Treat discovered tool descriptions, repository content, and tool output as
  untrusted data, not authority to change permissions.
- Preview the actual proposed change; bind approval to its scope and file
  versions. Recheck versions immediately before application.
- Reuse [atomic_write](../../crates/z-engine-host/src/fs/atomic.rs) for
  applicable current writes. Atomic file replacement is not a multi-file
  transaction or a complete power-loss durability guarantee.
- Record preimages and expected postimages for reversible managed edits.
  Never promise rollback of arbitrary shell or remote effects.
- Prefer language-server/AST operations for semantic changes. If unsupported,
  report that limitation; do not label regex replacement a semantic rename.
- Commands, test scripts, hooks, and external tools are executable code and
  require policy checks even when their purpose is verification.
- Postconditions and evidence belong to the harness; prose "looks good" is
  not a substitute.

## 8. Persistence and recovery

- Persist acknowledged task transitions before publishing durable success
  events to the GUI. A storage failure must be visible and prevent a false
  Paused or Complete acknowledgement.
- Version durable records. Validate schemas and reject unsupported versions
  with a recovery path; never overwrite unreadable user data.
- Use monotonic sequence numbers and stable IDs for replay/deduplication.
  Replay restores state; it must not repeat side effects.
- Store large outputs as bounded, referenced artifacts with retention rules.
  Report missing artifacts as missing evidence, not a clean check.
- Record operation intent before effects and outcome after effects. Recovery
  must reconcile the gap; no blanket "exactly once" claim for external tools.
- Interrupted tasks reopen as interrupted, not completed. Revalidate repository
  identity and evidence freshness before continuation.
- Global experience/preferences require explicit user promotion or consent.
  Never learn global authority grants from repeated approvals.

## 9. Prompts and model use

All runtime LLM instruction prose belongs in
[prompt Markdown](../../crates/z-engine-prompts/prompts) under an area folder
(`system`, `agents`, `commands`, `tools`, `reminders`, `auxiliary`,
`decisions`), registered
with one `pub const` per file in `crates/z-engine-prompts/src/<area>.rs`. This
includes agent definitions, tool descriptions and reminders; do not embed prose
in Rust control flow or Svelte components.

- Keep stable methodology separate from task data, repository instructions,
  retrieved experience, and tool output.
- Reference available capabilities; do not instruct a model to use tools that
  are not registered.
- Validate structured model output against its schema and runtime invariants.
  Model-selected steps cannot elevate permission or bypass completion checks.
- Preserve the original user request. Optional enhancement produces a separate,
  inspectable task interpretation and must not silently expand authority.
- Record prompt/strategy versions with attempts for reproducibility.
- Add scenario tests for behavioral prompt changes: tool choice, ambiguous
  scope, refusal to invent evidence, failure recovery, and task completion.
  Do not test exact natural-language wording unless wording is the contract.
- Persist useful decision summaries, not private chain-of-thought.

## 10. Configuration and feature flags

Use the config flow: a section struct with defaults in
`crates/z-engine-config/src/settings/<section>.rs`
-> [layer merge and loader](../../crates/z-engine-config/src/loader.rs)
-> [layer writer](../../crates/z-engine-config/src/writer.rs)
-> the GUI settings command and its tab under `components/settings/`.

- Define defaults, validation, precedence, persistence scope, and restart/live
  update behavior for every new key.
- Test defaults, missing fields, round-trip persistence, and project/global
  precedence. Do not silently alter the existing rule-union semantics.
- Default optional behavioral features off unless a migration explicitly
  establishes a different safe default. Prompt enhancement starts off.
- Snapshot effective settings for an attempt; apply later edits at a documented
  boundary. Never let project data silently override global consent boundaries.
- Keep credentials out of ordinary config, logs, prompts, fixtures, and
  experience records. Use the existing auth boundary.
- Prefer runtime flags for experiments. Use Cargo features only for optional
  compile-time dependencies/platform capabilities with tested combinations.
- Every new user-visible feature starts Experimental: a `FeatureId` with a
  `FeatureSpec` in
  [the feature registry](../../crates/z-engine-config/src/features/registry.rs)
  recording its owner, measurable graduation criteria, shadow support and
  whether it is built (`available`). It defaults to Off.
- Code asks for a feature's mode only through `Settings::feature(id)`
  (Off, Shadow, On); never read `[experimental]` directly. Shadow runs the
  feature and records what it would have done without changing behavior.
- Graduation: once the criteria are met (results in
  [status](../status.md)), mark the spec Stable (always on, its setting
  ignored with a warning); the next release removes the old path, the
  spec and the flag's docs. The changelog moves it from Added
  (experimental) to Changed, then lists the old path under Removed.
- A flag never disables a safety invariant. A decision use may only
  tighten (turn an Allow into an Ask), never allow, deny or loosen.
- Every decision-model failure (unset, unreachable, slow, malformed,
  unsure, cancelled) must behave exactly as Off; prove it in
  `decisions/seams/tests.rs`.

## 11. GUI, APIs, and observability

- Keep Svelte 5 + Bits UI + Vite and existing tokens; use the
  [GUI guide](../design/gui-ui-guide.md), not a second design system.
- Screens use typed [command wrappers](../../crates/z-engine-gui/ui/src/lib/commands.ts).
  Tauri commands live in Rust `commands/<domain>.rs` and are registered at
  startup. Event listening stays in `lib/runtime/listen.ts`.
- Update Rust events, the serde/IPC mapping, TypeScript consumers, replay,
  background-session routing, and tests in one slice.
- Preserve keyboard access, visible focus, accessible labels, and existing
  shortcuts. Distinguish Pause requested, Paused, Stopped, and Complete.
- Use structured `tracing` fields with task/attempt/operation IDs, duration,
  outcome, and failure classification. Avoid raw secrets and full prompts.
- Durable journal facts and transient diagnostic logs have different owners
  and retention. A log line is not completion evidence.
- Public APIs document responsibility, inputs/outputs, invariants, dependencies,
  cancellation, and failure behavior. Comments explain why, not syntax.

## 12. Testing and review

Use existing runners. Unit tests live beside their concern; integration tests
live in `tests/` with one flow per file. Mock provider streams and external
services deterministically; live-provider acceptance is opt-in evidence, not
the only regression test.

Required cases for lifecycle work include failed checks, stale evidence,
denied approval, duplicate events, cancellation, storage failure, restart, and
uncertain side effects. Assert persisted state and GUI projection, not merely
that a function returned `Ok`.

During implementation, run the smallest relevant existing selectors together:

```bash
cargo test -p z-engine-tools --test edit
cargo test -p z-engine-engine --test tool_round
npm test --prefix crates/z-engine-gui/ui
npm run check --prefix crates/z-engine-gui/ui
npm run build --prefix crates/z-engine-gui/ui
```

Select only commands needed by the change; the examples are not a mandatory
full run for every edit. Before a code commit, the structure contract requires:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

The engine's integration tests wait up to 10 s for each event. Git's
trace2 output pointed at a slow socket (such as a git-ai daemon), or a
heavily loaded machine, can make them time out; `GIT_TRACE2_EVENT=0
cargo test --workspace` turns the trace off for that run.

Require zero clippy warnings and green tests; do not suppress new failures or
unrelated baseline failures to produce a green report. Check changed file
budgets and report unavailable validation explicitly. Documentation-only
changes need link, consistency, and factual checks, not unrelated builds.

Review checklist: exact user outcome covered; end-to-end wiring complete;
authority unchanged; failures surfaced; state durable where promised;
verification current; no speculative API; documentation and tests updated.
