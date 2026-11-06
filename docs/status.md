# v2 rewrite status

The v2 rewrite was built on the `v2` branch in phases and merged into
`release` as version 2.0.0. Since the cutover (phase 5) the v1 crates are
gone; they remain in git history.

| Phase | Scope | Status |
|---|---|---|
| 0 | Branch, crate skeletons, protocol, llm core types, testkit, prompts skeleton | done |
| 1 | LLM adapters (OpenAI-compatible, Anthropic), retries, fallback, catalog, cost | done |
| 2 | Config v2 + v1 import, policy engine, host adapters | done |
| 3 | Tools | done |
| 4 | Engine core (single agent) | done |
| 5 | GUI cutover, v1 parity, delete v1 crates | done |
| 6 | Multi-agent orchestration, worktree isolation | done |
| 7 | MCP, LSP, rules, repo map, caching | done |
| 8 | Verification (discovery, evidence, badges, modes) | done |
| 9 | Commands, MCP prompts, workspace trust | done |
| 10 | Sandbox, fault injection, flakiness, performance, docs, 2.0.0 | done |

## GUI redesign (shipped as 2.1.0)

| Step | Scope | Status |
|---|---|---|
| 1 | Dead CSS and components removed; tokens, base, motion and materials in `ui/src/styles/`; native window translucency | done |
| 2 | Title bar, sidebar, content sheet, scroll-edge fades, one-home-per-fact cleanup | done |
| 3 | Title bar island with the companion, the waiting and context satellites and the island sheet (replaces the status line and Now card); sidebar with Home, Inbox and Projects; `ui.companion` setting | done |
| 4 | Transcript, tool grouping, turn receipt (`ui.task_report_view`), approval card, composer | done |
| 5 | Splash, first-run setup, project home and the Inbox (background notifications) | done |
| 6 | Agents panel, Changes panel, prompt inspector, palette, every Settings tab, worktree dialog, shell drawer | done |
| 7 | Errors after edits (engine event) | pending |
| 8 | Remove legacy stylesheets and token aliases | done |

## After 2.1.0: the pet (shipped as 2.2.0)

| Item | Status |
|---|---|
| The pet replaces the title-bar companion orb (`components/pet/`, pure rules in `lib/domain/pet/`); liveliness `ui.companion` moved to **Settings → Pet** | done |
| Roaming between perches (home spot, composer, sidebar footer, the side panel's tab band, empty Inbox), drag, boop, nap, card | done |
| Growth: XP, levels, stages, accessories, tricks, saved in `<data dir>/pet.json` (`pet_load`, `pet_save`) | done |
| `[ui.pet]` settings (`name`, `look`, `roam`), first-run **Meet** step, palette actions | done |
| One Send/Stop button, helper sprites, inspector ring chart, sliding sidebar selection, full-window page frame for Settings | done |
| Level-up celebration: confetti once nothing needs you (Lively only) | done |
| The pet as the logo: boot splash (the pet wakes up, then flies into its place), app icons, favicon and About logo replace the Z mark | done |

## After 2.1.0: the v4 dark frosted redesign (shipped as 2.2.0)

| Item | Status |
|---|---|
| Always dark: four material layers, one glass recipe, springs (`lib/ui/springs.ts`), motion on position and opacity only; macOS dark HUD vibrancy, Windows 11 dark Mica, solid elsewhere | done |
| Side panel with **Changes**, **Plan**, **Agents** and **Context** tabs, replacing the separate panels and the full-window prompt inspector | done |
| Approvals and questions docked in the composer; the **Plan ready · Review** row and the Plan tab | done |
| Island shapes and the island card; turn actions, the work summary line, the live compaction row, the long-chat turn window | done |
| QA in a browser against a mocked backend | done |
| Performance profile in headless Chrome of a 1,000-turn chat (`__hyperform.longChat(1000)`, dev builds only) | done |
| Native checks: macOS vibrancy under `tauri dev`, Windows 11 Mica, a full Windows MSVC build, title-bar double-click on a real window | pending |

## After 2.2.0: the 3D pet (unreleased)

| Item | Status |
|---|---|
| The pet drawn in 3D with Three.js (`lib/pet3d/`, the only `three` import): one shared WebGL renderer, one frame loop that runs only while something plays, loaded lazily; pure helpers `keyframes`, `rig3d`, `faceShapes`, `faceLayout`, `portrait` in `lib/domain/pet/` | done |
| The flat SVG pet (`PetFlat`) while the 3D pet loads, without WebGL and after the context is lost; the boot splash and helper sprites stay flat | done |
| The island's portrait of the roaming pet (`IslandPortrait`), replacing the empty nest | done |
| Checks by hand in headless Chrome against the real app with a mocked backend: every perch (hero, composer, empty Inbox, sidebar, side panel, island), drag and drop, a hard throw (lands dizzy), the card, **Settings → Pet**, first run, every look, stage, accessory, prop and mood, the island portrait and its tone ring, the flat pet without WebGL and after a lost context; frame counts: none while idle or asleep (blinks only), about 30 fps for mood loops, one draw per change under Reduce Motion | done |
| Fixes from those checks: the frame loop skipped a motion's last frame after a slow frame (eyes left mid-blink); a Svelte warning on every roaming frame; closing the card with **Customize…** threw an error (`petGaze` focus handler); the portrait now follows a slumped, sunk or sleeping body and the seed's size, keeps both eyes in its window at the widest turn and shows no gap above the crown; pets redraw when the screen's scale changes, also under Reduce Motion | done |
| The same checks in the native Tauri window (`tauri dev`) | pending |

## After 2.2.0: experimental features and the decision layer (unreleased)

| Item | Status |
|---|---|
| Phase 1: the feature registry and `[experimental]` (Off, Shadow, On), **Settings → Experimental**; `[decisions]` with the sidecar, **Test connection**, trust gating and the opt-in dataset; the `z-engine-decisions` crate (rules, SystemOne and hybrid providers, calibration, cache, trace); the engine's decision service and seams; the Context tab's **Decisions** section | done |
| Context and cost uses: relevance-aware compaction, task-scoped history (the `TaskView` log record, the divider, **Include full history**), first-request context, file prefetch (`[decisions.prefetch]`), output trimming and search ranking (the Relevance seam) | done |
| Per-task routing: effort per task, the fast model with `[decisions.routing] allow_model_switch`, subagents that inherit their model, the route chip | done |
| Safety uses: risk review (gate and injection notes), secret screening (the request seam, placeholders), custom decision rules (`[[decisions.rules]]`), loop guard (`[decisions.loop_guard]`) | done |
| Verification uses: completion check (the **Claimed, not checked** badge), check selection | done |
| Suggestions and guidance: plan, standing-rule and review cards (`Suggested`, `ResolveSuggestion`), correction signal, skill and agent hints, repeated-question check | done |
| The app: pet mood from turn tone (`TurnToneJudged`), inbox priority (`UrgencyScored`, gating `Notification` hooks) | done |
| Native runtime: `decisions.runtime = "native"` behind the `onnx` Cargo feature; pinned `english`, `multilingual` and `typed-decisions` checkpoints; resumable, SHA-256-checked download in **Settings → Experimental** | done |
| SystemOne wire fixes: the request carries `model` and `max_len`; scale scores given as a choice index are normalized; choices keep their order; `max_len` is part of the cache key | done |
| End-to-end tests (`crates/z-engine-engine/tests/decisions_*.rs`, below) | done |
| Release builds with the native runtime (`--features onnx` in the release workflow) | pending |
| The live A/B test and the offline benchmark against a real model | pending: not run, no credentials |

### Graduation criteria

Each feature graduates when it meets its criteria in
`crates/z-engine-config/src/features/registry.rs`; results are recorded
here. Every feature is Experimental and Off by default; every decision
feature supports Shadow, and work plans and parallel tasks are Off or On
only (parallel tasks also need work plans).

| Feature | `[experimental]` id | Graduation criteria | Results |
|---|---|---|---|
| Relevance-aware compaction | `decisions_compaction` | At least 15% fewer context tokens at equal task success, zero loss of mandatory context, p95 at or under 250 ms, fallbacks at or under 5%. | pending |
| Task-scoped history | `decisions_task_view` | At least 30% fewer input tokens per turn on chats of 5 or more tasks, no drop in task success, readbacks at or under 5%, and cost per task no higher once prompt caching is counted. | pending |
| First-request context | `decisions_session_context` | Fewer tool rounds before the first useful read at equal task success, with no rise in first-request tokens. | pending |
| Per-task routing | `decisions_routing` | Lower cost per task at equal task success, with the share of input read from the prompt cache not lower than with the flag off. | pending |
| Loop guard | `decisions_loop_guard` | Catches at least 80% of the loop fixtures with at most one needless reminder per 50 turns. | pending |
| Completion check | `decisions_completion_check` | At least 90% precision on claim fixtures; a turn is never marked Verified by this feature. | pending |
| Risk review | `decisions_risk` | No decision ever loosened (enforced by structure) and at most one needless escalation per 50 calls. | pending |
| Relevant output trimming | `decisions_output_trim` | At least 20% fewer tool-output tokens with no rise in re-runs of the same command. | pending |
| Search ranking | `decisions_search_rank` | At least 25% fewer follow-up searches after a capped result. | pending |
| File prefetch | `decisions_prefetch` | Fewer tool rounds before the first edit, with at least 60% of prefetched files read or edited in the task. | pending |
| Plan suggestion | `decisions_plan_suggest` | At least half of the suggestions accepted, and at most one suggestion per 20 small requests. | pending |
| Correction signal | `decisions_user_signal` | At least 85% precision on labeled correction messages. | pending |
| Skill and agent hints | `decisions_hints` | Hinted skills or agents used in at least 40% of hinted tasks. | pending |
| Standing-rule suggestions | `decisions_memory_suggest` | At least half of the offers accepted. | pending |
| Repeated-question check | `decisions_question_check` | At least 90% precision on repeated-question fixtures. | pending |
| Review suggestion | `decisions_review_suggest` | At least 30% of suggestions accepted, and at most one suggestion per 10 turns. | pending |
| Pet mood from turn tone | `decisions_pet_mood` | At least 80% agreement with hand labels of turn tone. | pending |
| Inbox priority | `decisions_inbox_priority` | At least 80% agreement with hand-ranked urgency, with no urgent item left without a notification. | pending |
| Check selection | `decisions_check_select` | Check time cut by at least 30% with no failing check skipped. | pending |
| Custom decision rules | `decisions_custom_rules` | p95 at or under 250 ms, and no rule ever changes a permission (enforced by structure). | pending |
| Secret screening | `decisions_secret_screen` | At least 95% recall on credential fixtures with at most one false alarm per 100 tool results. | pending |
| Work plans | `work_plans` | On the long-task fixtures: fewer tasks wrongly claimed done, no rise in tokens per task, the full packet re-sent only on a new revision, and requests identical to the flag off when no plan is attached. | pending |
| Parallel tasks | `work_parallel` | On the parallel fixtures: less wall time than serial with no rise in failed or reopened tasks, no change applied without your approval or a passing integration check, and every write outside a task's scope asked first. | pending |

### Decision layer tests and benchmarks

End-to-end tests in `crates/z-engine-engine/tests/` (run by
`cargo test --workspace`):

- `decisions_all_on.rs`: every feature On against a decision model that
  always answers confidently with the answer that acts, over a scripted
  session (two tasks, approvals, a denied command, a stop with changes, a
  compaction, a question). It completes with no lost events, every call the
  policy asks about still asks, a denied call never runs or asks, no turn
  is cancelled and no turn is marked Verified.
- `decisions_fallback.rs`: every feature On against a decision model that
  is down or never answers in time; the main model gets the same requests,
  the same calls ask and finish the same way, and verification ends as with
  every feature Off.
- `decisions_shadow.rs`: every feature in Shadow against a confident model;
  the session runs exactly as with every feature Off while the trace
  records what each feature would have done, marked as shadow.
- One file each for compaction, task view, first-request context, loop
  guard, secret screening and the repeated-question check.

The live A/B test (ignored) runs small fixture tasks against a real model
twice, every feature Off and then the chosen features On, and prints
tokens, cost and tasks solved per arm. Results: pending (not run; no
credentials here).

```bash
HYPERFORM_MODEL=... HYPERFORM_API_KEY=... HYPERFORM_DECISIONS_URL=... \
  cargo test -p z-engine-engine --test decisions_ab_live -- --ignored --nocapture
```

Optional: `HYPERFORM_PROVIDER`, `HYPERFORM_BASE_URL`, `HYPERFORM_DECISIONS_KEY`,
`HYPERFORM_DECISIONS_ALLOW_REMOTE=1` and `HYPERFORM_AB_FEATURES`
(comma-separated ids; default every available feature).

The offline benchmark (`crates/z-engine-decisions/tests/eval.rs`)
always validates the hand-labeled fixtures; with `HYPERFORM_DECISIONS_URL`
pointing at a laya-serve, its ignored run compares rules, the raw model
and the hybrid gate per question kind, and prints a fitted
`[decisions.calibration.<question>]` block per kind to paste into
`settings.toml`. Results: pending.

```bash
HYPERFORM_DECISIONS_URL=http://127.0.0.1:8000 \
  cargo test -p z-engine-decisions --test eval -- --ignored --nocapture
```

Optional: `HYPERFORM_DECISIONS_KEY`, `HYPERFORM_DECISIONS_CHECKPOINT`,
`HYPERFORM_DECISIONS_ALLOW_REMOTE=1`, `HYPERFORM_DECISIONS_THRESHOLD` (default
0.8) and `HYPERFORM_DECISIONS_CALIBRATION` (`question=temperature[:threshold],...`).

### Known gaps

- Only the main conversation is secret-screened; titles, summaries and
  other side requests to the fast model are not.
- The **Claimed, not checked** badge, route chips and suggestion cards are
  live only; they are not kept when a chat is reopened.
- There are no operating-system notifications: inbox priority only sorts
  the Inbox and gates the user's own `Notification` hooks (they fire for
  high urgency or when unscored).
- The native model pins are community ONNX exports
  (`onnx-community/laya*-ONNX`) at pinned revisions, not files published
  by Laya's authors; `NOTICE` covers attribution.
- The native runtime is not the default, and release builds need
  `--features onnx` (the release workflow doesn't pass it yet).
- Some parts act without the decision model by design: the loop guard's
  repeat detection, secret screening's pattern detectors and review
  suggestion's path rules. Everything that needs the model behaves as Off
  when it fails, times out or is unsure.

## After 2.3.0: GitHub Copilot provider (unreleased)

| Item | Status |
|---|---|
| OpenAI Responses adapter (`z-engine-llm/src/responses/`), used only by Copilot | done |
| Copilot client (`z-engine-llm/src/copilot/`): GitHub device sign-in with OpenCode's OAuth app, `/models` routing to Messages, Responses or chat completions with a by-name fallback, Copilot headers, sign-in-again hint on 401/403 | done |
| `github-copilot` credential bucket (Enterprise: `custom:copilot-api.<domain>`), no provider environment variable | done |
| Sign-in in **Settings → Providers** and the setup guide (`CopilotSignIn`), `copilot_login_start` / `copilot_login_poll` commands | done |
| Integration tests on the mock server (`tests/copilot_client.rs`, `tests/copilot_login.rs`) | done |
| Cost, context window and output limit from the active provider's catalog entry first (`catalog_provider` + `ModelCatalog::lookup_in`), so Copilot models use the `github-copilot` listing | done |
| A live check against a real Copilot account | pending |

Known gaps: the model picker lists the models.dev `github-copilot` catalog,
not the account's live list; cost comes from that catalog's `github-copilot`
price for the model (another provider's when it has none), not Copilot's
plan billing; subagent and compaction requests are not marked
`x-initiator: agent` unless they follow a tool round.

## After 2.5.0: plan mode as a switch (unreleased)

| Item | Status |
|---|---|
| Session `planning` separate from `PermissionMode { Default, AcceptEdits, Bypass }`; `Policy::decide(tool, action, planning, mode)` with the planning floor before bypass (unclassifiable commands denied) | done |
| Compatibility through `ModeName`: old logs (`modeChanged` with `mode: "plan"`), `permissions.mode = "plan"` and `permissionMode: plan` read as planning at Default; new `planningChanged` record, `setPlanning` command, `planningChanged` event, `SessionInfo.planning` | done |
| Build keeps the level (`PlanDecision::Approve { mode: None }`), subagents inherit planning, hooks still see `permission_mode: "plan"`, a project may turn planning on before trust | done |
| GUI: removable **Plan** chip, three-level permission chip, Shift+Tab toggles plan mode, one **Build** button, **Start new chats in plan mode** setting | done |

## After 2.5.0: work plans, milestone 1 (unreleased)

Behind `work_plans` (Experimental, Off or On). How it works:
[work plans](how-it-works/features-work-plans.md).

| Item | Status |
|---|---|
| `z-engine-work`: task graph, state machines, authority (`decide`), completion gate, propagation, packet selection, reconciliation; no I/O | done |
| Plan logs `plans/<project key>/<plan id>/` (`PlanStore`, schema 1); `PlanHub` with one writer per plan; read-only after a failed write | done |
| Protocol: `workUpdated`, `taskPacketSent`, `editWork`, `attachPlan`, `detachPlan`, `PlanDecision::Approve { handoff }`, `PlanAttached`/`PlanDetached` log records | done |
| Tools `TaskCreate`, `TaskUpdate`, `TaskGet`, `TaskList`, `PlanRecord` (only while On); evidence linked from recorded checks; attempt limit | done |
| Task packets each turn, the plan restored after compaction, `/replan`, reconciliation on workspace changes; `[work]` settings | done |
| GUI: the Plan tab (tasks, criteria, notebook, change set), **Build in a new chat**, the Context tab's **Task packet**, trace rows in the transcript | done |

## After 2.5.0: work plans, milestone 2 (unreleased)

Helpers on tasks and the plan audit come with `work_plans`; running tasks
side by side is behind `work_parallel` (Experimental, Off or On, needs
`work_plans`). How it works:
[parallel tasks and the plan audit](how-it-works/features-work-parallel.md).

| Item | Status |
|---|---|
| `Agent { task_id }` (main agent only): kernel dispatch check, the helper's brief and result contract, the worker role, attempts forgiven by your retry | done |
| `TaskUpdate { report }` folded with observed files and checks; `Blocked { Unreported }` without a report | done |
| Parallel eligibility (`ParallelSet`, `work_parallel` query), approval only by you, `[work]` `max_parallel`, `merge`, `serial_paths` | done |
| Merging back one at a time (`ask` or `auto`), integration checks in the project tree (and the main agent's checks) with **Run checks again** (`recheckTask`), conflicts kept for **Retry serially**, discards blocking the task; task worktrees kept on open | done |
| The coordinator reminder after every helper outcome; superseded dependencies block their dependents | done |
| Scope drift: a helper's write outside its task's scope asks and is noted | done |
| Trust: an untrusted project can't turn on `merge = "auto"` or drop serial paths | done |
| The deterministic audit on every revision, the `audit` agent (deep plans, **Run audit**), **Add to plan** and **Dismiss**, plan health | done |
| GUI: **Needs you**, **Now** lanes, the coverage line and findings, the task **Result**, parallel badges and the build and plan menus, task names on helper rows and cards | done |

## After 2.5.0: work plans, milestone 3 (unreleased)

Pausing, resuming and moving plans, behind `work_plans`. How it works:
[pausing, resuming and moving plans](how-it-works/features-work-resume.md).

| Item | Status |
|---|---|
| A failure holds only what it affects: findings make the plan Replanning but hold only the tasks they name and their dependents (`held.rs`, `Readiness::ReplanHeld`, `ParallelBlock::Replanning`, `replanHeld`); replanning you start still holds everything (changes milestone 2) | done |
| Pause, hold (`holdTask`, `releaseTask`), Stop and Abandon: soft stops at a round boundary with resume pointers, hard stops that cancel; closing a chat, detaching and moving soft-stop; Done by itself, **Reopen** | done |
| Resume on open or attach: interrupted tasks with resume pointers (`TaskInterrupted`, forgiven attempts), outside changes since the last linked checkpoint reconciled, one `resume` plan change, the resume reminder and packet; `Agent { task_id, resume }` | done |
| Checkpoints linked after each main turn and each applied helper change (`CheckpointLinked` with a shadow snapshot) | done |
| One chat drives a plan: attaching moves it, "This plan moved to another chat."; `PlanSummary` `blocked`, `needsYou`, `running`, `lastSessionId`, `attachedTo` | done |
| "Needs you" counts high-severity audit findings only (`needs.rs`) | done |
| GUI: the state banner, task holds and interrupted notes, the **Continue a plan** picker (Plan tab, composer **+**, palette, plan menu), the home card, one Inbox item per plan, per-plan view state, interrupted helpers in the Agents tab, the **Plan resumed** row | done |
| Browser QA against a mocked backend: `npm run dev --prefix crates/z-engine-gui/ui` outside Tauri installs `lib/runtime/dev/` (dev builds only) | done |
| Engine integration tests: `work_restart.rs` (interrupted helper, outside edit, quiet reopen, `Agent { task_id, resume }`), `work_pause.rs` (pause, resume, hold, release, stop), `work_keep_running.rs` (a failure holds only its dependents), `work_attach.rs` (moving a live or closed chat's plan, `attachedTo`, needs-you) | done |
| Helpers in one batch are admitted one at a time under a per-plan lock, so the parallel approval, `max_parallel` and overlap checks see the helpers admitted before them (fixed a race where two could pass together) | done |
| A scope path that doesn't exist yet (a file its task will create) is no longer reported as deleted; deletions come only from the checkpoint diff | done |

Graduation: `work_plans` and `work_parallel` stay Experimental. Their
criteria ([above](#graduation-criteria)) need the long-task and parallel
fixtures, which don't exist yet, and runs against a real model; until then
both results stay pending.

Known gaps:

- `curl` through `Bash` and MCP tools don't count as reading the web.
- An outside edit right after a task's completion turn isn't flagged as
  stale evidence.
- A plan that never linked a checkpoint falls back to its latest check
  fingerprint and may show a false "changed outside the plan" once.
- The Inbox and the home read the work-plans switch from the active
  project's settings.
- The "Next" line under the **Now** lanes and the elapsed time in the plan
  header aren't built.
- Focus doesn't return after the narrow panel's back.
- Turning planning on while findings already replan the plan keeps the
  findings' hold instead of holding every task (plan mode still runs
  nothing).
- A task interrupted because its plan moved is counted in the next
  resume's summary, not the new chat's first one.
- After a move or detach, the old chat's Agents tab still offers Apply on
  that plan's helpers; the engine refuses it.
- The "This plan moved to another chat." row isn't kept across a restart
  (notices aren't stored).

## After 2.0.0: documentation website

- VitePress site at [arshadbarves.github.io/hyperform](https://arshadbarves.github.io/hyperform/),
  built from `docs/` and deployed by `.github/workflows/docs-site.yml` on
  pushes to `release`.

## Release checks (2.0.0)

- `cargo clippy --workspace --all-targets -- -D warnings`: no warnings.
- `cargo test --workspace`: 1268 passed, 0 failed, on two consecutive runs
  (the second alongside an extra engine suite for CPU load); the engine
  suite also passed five repeated runs.
- Frontend: 303 vitest tests, `svelte-check` 0 errors, `oxlint` clean,
  production build succeeds.
- Timing harnesses (release): a 5,000-message, 22 MB session opens in about
  55 ms; token estimation over 2,000 messages takes about 4 ms.

## After 2.0.0: documentation and review fixes

| Item | Status |
|---|---|
| User guide (`docs/user-guide/`) | done |
| How HyperForm works (`docs/how-it-works/`) | done |
| Documentation contract (`docs/AGENTS.md`), `docs-maintainer` subagent, CI link check | done |
| Untrusted workspaces can only make settings stricter; sessions and the settings screen share one rule | done |
| `AGENTS.md` corrected: network exceptions, steps for adding a tool, updater listener | done |
| Custom check ids in `auto_checks`, turning off built-in language servers, daily catalog refresh, `fetch_extract` wording | done |

Open findings from the user-guide review:

- Settings, agents, commands and skills edited outside the app reach an
  open chat only when it is reopened or something is saved in **Settings**:
  there is no file watcher, and `reloadExtensions` has no caller.
- Skill `allowed-tools` and command `disable-model-invocation` are parsed
  but have no effect.

## Decisions and notes

- Protocol and config TypeScript is generated by `ts-rs` into
  `crates/z-engine-gui/ui/src/lib/protocol/` via `.cargo/config.toml`;
  regenerate with `cargo test -p z-engine-protocol -p z-engine-config -p z-engine-llm`.
- `z-engine-tools` depends on `z-engine-policy` for the `Action` type used
  in permission decisions (documented in AGENTS.md).
- The Tauri shell depends only on `z-engine-engine`, `z-engine-protocol` and
  `z-engine-config`; GUI-only queries live in `z-engine-engine/src/engine/queries/`.
- v2 never modifies v1 `config.toml` files; it reads `settings.toml` and
  imports v1 settings. Spent cost stays in session totals after rewinds.
- The v1 design documents have been removed; git history keeps them.
