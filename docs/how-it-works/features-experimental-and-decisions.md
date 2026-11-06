# Experimental features and the decision layer

How a new feature is tried before it becomes standard, and the decision
layer that the decision features build on: a small model the engine may
ask short questions, with today's behavior as the answer whenever the
model can't help. What each of the 21 decision features does is in
[Decision uses](features-decision-uses.md). How to use them:
[Experimental features](../user-guide/15-experimental-features.md) and
[Decision features](../user-guide/16-decision-features.md). Terms are in
the [glossary](glossary.md).

## Experimental features and graduation

**In plain words.** A new feature starts on probation, like a new dish
offered as a tasting before it joins the menu. You choose whether it stays
off, runs quietly and takes notes, or acts; once it meets a bar written
down in advance, it joins the menu for good.

**How it works**
1. Every feature has an id and one entry in a registry: title, summary,
   stage (Experimental or Stable), group, whether it can run in
   [shadow mode](glossary-experimental.md#shadow-mode), whether it is built in this
   version, an owner, and its graduation criteria.
2. You set a [feature flag](glossary-experimental.md#feature-flag) per id in
   `[experimental]`: Off (default), Shadow or On. The engine asks one
   function for a feature's effective mode:
   - a stable feature is always On (a setting for it is ignored with a
     warning);
   - a feature not built in this version is Off (Shadow or On warns);
   - Shadow on a feature without shadow support is Off (with a warning);
   - otherwise the configured mode.
3. **Settings → Experimental** lists only features that are built and
   still experimental, so a stable feature has no switch.
4. [Graduation](glossary-experimental.md#graduation): when a feature meets its
   criteria (results recorded in
   [status](../status.md#graduation-criteria)), its stage becomes Stable
   and it is always on. The next release removes the old path, the
   registry entry and the flag's documentation. The changelog lists it
   under Added (experimental), then Changed (stable), then Removed (the old
   path). Decision features keep today's rules as the runtime fallback;
   only the flag goes.
5. This version registers 23 features, all built and Experimental, none
   graduated yet: 21 decision features (`decisions_*`), all with shadow
   support, [work plans](features-work-plans.md) (`work_plans`) and
   [parallel tasks](features-work-parallel.md) (`work_parallel`), which
   have none and don't use the decision layer.
6. A feature may require another (`FeatureSpec::requires`): it is Off
   while that one isn't running, and a settings file that turns it on
   alone gets a warning. `work_parallel` requires `work_plans`.

**For developers**
- [`features/id.rs`](../../crates/hyperform-config/src/features/id.rs)
  (`FeatureId`, its `[experimental]` key),
  [`mode.rs`](../../crates/hyperform-config/src/features/mode.rs)
  (`FeatureMode`: `runs()` for Shadow or On, `acts()` for On only) and
  [`registry.rs`](../../crates/hyperform-config/src/features/registry.rs)
  (`FeatureSpec`, `FEATURES`, `.available()`).
- [`settings/experimental.rs`](../../crates/hyperform-config/src/settings/experimental.rs):
  `Settings::feature(id)`, the only way code asks, `running_features()`,
  and the warnings; unknown ids are reported by `settings/unknown_keys.rs`.
- The GUI lists `FEATURES` from the `feature_catalog` command
  (`listedFeatures()` in `ui/src/lib/domain/settings/features.ts`).
- A new feature: a `FeatureId` variant (in `ALL` and `as_str`), its
  `FeatureSpec`, code gated on `settings.feature(id)`, then
  `cargo test -p hyperform-config` to regenerate the TypeScript.

## The decision layer

**In plain words.** Some choices the engine makes by fixed rules could use
a quick second opinion. The decision layer asks a small, fast model short
questions; the model proposes and the engine decides. It is like a junior
colleague whose advice you take only when they are sure, and ignore when
they don't answer in time.

**How it works**
1. A [decision use](glossary-experimental.md#decision-use), one per decision feature,
   turns a moment of a turn into typed questions: a choice between named
   options, a yes or no, or a level on a scale. All questions about one
   state travel in one request.
2. Each session has a decision service, built from its settings and
   rebuilt when they change: the running features with their modes, and a
   provider that answers.
   - With a usable [decision model](glossary-experimental.md#decision-model) (a
     server, a sidecar or the native model), the *hybrid* provider
     answers.
   - Otherwise the *rules* provider answers, and it always abstains.
3. The hybrid provider first looks in a cache of 512 answers, keyed by the
   question, its input and the model's revision (its address or pinned
   revision, checkpoint and input length). The rest go to the model in one request (split by `max_batch`),
   bounded by `timeout_ms`. Calibration then rescales the confidence per
   question, and an answer below its threshold (default 0.8) abstains as
   *low confidence*.
4. Any failure abstains too, with its reason: `timeout`, `unavailable`
   (unreachable, an HTTP error, or a native model still loading),
   `malformed`, `cancelled` or `invalid`. Each counts as a
   [fallback](glossary-experimental.md#fallback-decision).
5. An abstained answer means "keep today's behavior". So a feature that is
   On with a model that is down, slow, confused or unsure does exactly what
   it does Off.

**For developers**
- The crate [`hyperform-decisions`](crates.md#hyperform-decisions):
  [`question.rs`](../../crates/hyperform-decisions/src/question.rs)
  (`Question`, `Form`, `DecisionRequest`),
  [`answer.rs`](../../crates/hyperform-decisions/src/answer.rs) (`Answer`,
  `Verdict`, `AbstainReason`),
  [`provider.rs`](../../crates/hyperform-decisions/src/provider.rs) (the
  async `DecisionProvider` trait, cancellable), and
  [`providers/`](../../crates/hyperform-decisions/src/providers/):
  `rules.rs`, `systemone/`, `onnx/` and `hybrid.rs`, with
  `calibration.rs` and `cache.rs` beside them.
- The wire format (`providers/systemone/wire.rs`):
  `POST <endpoint>/v1/systemone` with a `state`, the `model` (checkpoint)
  and `max_len` (both left out when blank or 0), and one entry per
  question under `questions` (`type`, `instructions`, `criteria`, choices
  in their given order). Yes/no questions go out as a two-option choice;
  replies are read from `answers.<name>` (`choice` or `score`,
  `probabilities`, `answer_confidence` or `confidence`). A scale score
  given as a choice index is divided by the number of levels minus one;
  unknown fields are ignored and `act_probability` is never read.
- Question wording lives in
  [`prompts/decisions/`](../../crates/hyperform-prompts/prompts/decisions/):
  the instructions, then one `- key: description` line per option, with a
  `pub const` and an `ALL` entry in `src/decisions.rs`: 33 files, one per
  question plus `connection-probe.md`. Custom rules bring their own
  questions, named `rule_<slug>`.
- In the engine, [`decisions/service.rs`](../../crates/hyperform-engine/src/decisions/service.rs)
  (`DecisionService`), [`hub.rs`](../../crates/hyperform-engine/src/decisions/hub.rs)
  (`DecisionHub` on `SessionCore`: the service, swapped on reload, plus
  the trace, the shadow slots, the per-feature memory and a memo of what
  was already said, kept) and
  [`context.rs`](../../crates/hyperform-engine/src/decisions/context.rs)
  (`UseContext::ask`, `record`).

## Seams: where a decision may act

**In plain words.** Decision uses can't reach in anywhere. They may speak
up only at fixed points of a turn, and at each one only in ways that keep
you safe, like a passenger who may point out a turn but can't grab the
wheel.

**How it works**

| Seam | When | An On use may | It can't |
|---|---|---|---|
| Turn start | your message arrived, after `UserPromptSubmit` hooks | add reminders to the turn's opening message | change your message |
| Context pressure | old tool results are about to be cleared ([compaction](features-agents-and-context.md#compaction)) | keep planned clears or clear more; keeping wins | touch the summary compaction |
| Before a request | a request is about to reach the provider | flag likely credentials in tool results not screened yet, for an approval card | remove anything without your Deny |
| Tool gate | the policy, or a hook, allowed a call | turn that Allow into an Ask, with a reason and no rule to save | allow, deny, or change an Ask or a Deny |
| After a tool call | a call finished (not cancelled), after `PostToolUse` hooks | put notes ahead of the result | change the output |
| Stop boundary | the main agent stopped and verification has nothing more to ask | send the agent back with one reminder, at most once per turn | end a turn or set its badge |
| Ask user | `AskUserQuestion` is about to reach you | answer with a pointer to your earlier messages instead | keep a repeated question from reaching you |
| Route | a task starts: the main agent's new task, or a subagent whose model is `inherit` | pick an effort you left unset, or the fast model when allowed | pick any other model, or override an effort or model you chose |
| Completion | the main agent stopped with changes no check backs | report a success claim to verification | change the badge or a check record |
| Check select | auto or strict verification is about to run its checks | name checks the changes can't affect | skip anything when a test file changed, or leave strict without a test, build or typecheck check |
| Relevance | a tool is about to cut a long result (command output, search results) | say which parts matter for the task | change the result otherwise; without an answer the usual cut applies |
| Turn end | a main-agent turn ended (in the background) | judge its tone for the pet | delay or change the turn |
| Attention | something needs you, or a warning or error notice | rate how urgent it is | hide it; it only sorts the Inbox and gates `Notification` hooks |

1. A seam returns at once when no running use joins it; with every feature
   Off, nothing is built or started.
2. [Shadow](glossary-experimental.md#shadow-mode) uses run in the background, at most
   four per session at once; more are skipped, never queued. Their advice
   is dropped and only their trace records remain, so they never delay or
   change a turn. A shadow use that would show a card shows none.
3. On uses at a seam run together against one deadline, twice
   `timeout_ms` plus 50 ms. A use that misses it, or fails, gives no
   advice and leaves a trace record saying `timeout` or `invalid`.
4. Advice is applied only from On uses, in `USES` order. Where only one
   answer can win (a gate reason, a stop reminder, a route, a claim, a
   relevance ranking, an ask-user note, check skips, urgency scores), the
   first wins; notes, reminders and secret findings add up.
5. So a decision can only tighten: it adds an approval, a notice, a note
   or a card. It never allows, denies, cancels or hides.

**For developers**
- [`decisions/registry.rs`](../../crates/hyperform-engine/src/decisions/registry.rs):
  the `DecisionUse` trait (one method per seam, each defaulting to no
  advice), `Seam`, and `USES`, the 21 uses in registry order (a test keeps
  them equal to the `.available()` features).
  [`decisions/uses/`](../../crates/hyperform-engine/src/decisions/uses/)
  holds one file or folder per feature, named after its id without
  `decisions_`.
- [`decisions/seams/`](../../crates/hyperform-engine/src/decisions/seams/):
  `dispatch.rs` (shadow slots, deadline) and one file per seam, called from
  `session/turn.rs` (`at_turn_start`, `at_turn_end`), `run/pressure.rs`
  (`review_clears`), `run/agent.rs` (`screen_request`), `batch/gate.rs`
  (`review_call`, after `with_override`), `batch/call.rs`
  (`annotate_result`), `run/stop.rs` (`review_stop`,
  `MAX_DECISION_CONTINUATIONS = 1`), `ports/interaction.rs`
  (`review_questions`), `routing/start.rs` (`route_task`, also from
  `orchestration/launch.rs`), `verify/claims.rs` (`review_completion`),
  `verify/modes.rs` (`select_needed`), `ports/relevance.rs`
  (`rank_items`), `hooks/notify.rs` (`score_attention`,
  `worth_notifying`) and `session/open.rs` (`watch_notices`).
- Suggestion cards go through
  [`decisions/suggest.rs`](../../crates/hyperform-engine/src/decisions/suggest.rs):
  `offer` emits `Suggested` (not in shadow), and
  `Command::ResolveSuggestion` traces the click with provider `user`.
- The Off guarantee is tested in `decisions/seams/tests.rs` (a probe use
  against a model that is down, times out, answers garbage, is unsure or
  hangs) and [`tests/decisions_fallback.rs`](../../crates/hyperform-engine/tests/decisions_fallback.rs)
  (every feature On with a dead model sends the main model the same
  requests as every feature Off).
- A new use: implement `DecisionUse` in `uses/<name>.rs` for its seams, ask
  through `UseContext::ask`, record with `UseContext::record`, add it to
  `USES`, and mark its registry entry `.available()`. Every failure must
  behave as Off; extend `seams/tests.rs`.

## Reaching the decision model

**In plain words.** The decision model is either a separate small program
on your computer, or runs inside the app itself. HyperForm talks to one you
already run, starts one for you and stops it when you quit, or loads a
downloaded copy, like a coffee machine you switch on yourself, leave on a
timer, or have built into the kitchen.

**How it works**
1. With `runtime = "sidecar"` (the default) and `[decisions.sidecar]
   command` set, the engine starts that command as a
   [sidecar](glossary-experimental.md#sidecar): once for the whole app, through its
   background shells, in the data folder, on a free port on 127.0.0.1,
   with `LAYA_HOST`, `LAYA_PORT`, a fresh random `LAYA_API_KEY`,
   `LAYA_CHECKPOINT` and `LAYA_MAX_LEN`. Sessions that ask for the same
   command, checkpoint and length share it; a different one replaces it;
   quitting the app stops it. It starts only when a decision feature runs
   or on **Test connection**.
2. Without a sidecar, the engine uses `endpoint`. It must be on this
   computer (`localhost` or a loopback address) unless `allow_remote` is
   set, and may not carry a user name or password; `api_key_env` names the
   variable whose value is sent as `Authorization: Bearer`.
3. With `runtime = "native"`, the [native runtime](glossary-experimental.md#native-runtime)
   loads the checkpoint's pinned files from
   `<data dir>/models/laya/<revision>/` and answers in the app through
   ONNX Runtime; nothing goes over the network. Each checkpoint
   (`english`, `multilingual`, `typed-decisions`) pins a Hugging Face
   repository, a revision and a SHA-256 checksum per file, and caps
   `max_len` (512, 8192 and 1024 tokens).
   - The files download only from **Settings → Experimental**, one model
     at a time, through the host crate: each streams to `<name>.part`,
     resumes with an HTTP range request, and is renamed only when its size
     and checksum match.
   - Before loading, every file is checked against its checksum again. The
     model loads once for the whole app, on a background thread; until it
     is ready, questions abstain as `unavailable`.
   - It works only in builds with the `onnx` Cargo feature, which the
     release builds don't enable yet.
4. If a feature runs but the model can't be set up (the sidecar did not
   start, the key variable is unset, the endpoint is invalid or remote,
   the build lacks the native runtime, the native model isn't downloaded),
   the session shows "The decision model is not used: *reason*.
   Experimental decisions keep today's behavior." and uses the rules
   provider. A server that is set up but doesn't answer shows up later as
   fallbacks.
5. In an untrusted workspace the whole `[decisions]` table comes from the
   user layer; when the project sets it differently, workspace trust names
   "decision model" or "decision rules" among the settings it holds back.
6. **Test connection** asks the model directly (no cache, no calibration)
   one known yes/no question, "Does this text greet someone?", about a
   greeting, and waits up to 10 seconds (for a loading native model too).
   It reports the latency and confidence, an answer slower than
   `timeout_ms`, a wrong answer (check the checkpoint), the error, or that
   a model started moments ago may still be loading.

**For developers**
- [`decisions/build.rs`](../../crates/hyperform-engine/src/decisions/build.rs)
  (`build_service`, `connect`, the notice),
  [`sidecar.rs`](../../crates/hyperform-engine/src/decisions/sidecar.rs)
  (`Sidecars` on `Shared`, `WARM_UP`; `Engine::shutdown` stops it); the
  free port from `free_loopback_port` in
  [`host/src/net.rs`](../../crates/hyperform-host/src/net.rs).
- Native: the pins in
  [`native_model.rs`](../../crates/hyperform-decisions/src/native_model.rs)
  (`NativeModel`, `native_model`, the aliases), the provider in
  `providers/onnx/` (feature `onnx`: `ort` and `tokenizers`), the engine
  side in [`decisions/native/`](../../crates/hyperform-engine/src/decisions/native/)
  (`NativeRuntime` on `Shared`, `connect`, `files.rs`) and the download in
  [`host/src/download.rs`](../../crates/hyperform-host/src/download.rs).
  The feature chain is `hyperform-gui/onnx` → `hyperform-engine/onnx` →
  `hyperform-decisions/onnx`.
- The loopback rule is `providers/systemone/endpoint.rs`; the probe is
  [`probe.rs`](../../crates/hyperform-decisions/src/probe.rs), run by
  `Engine::test_decision_model` in
  [`engine/queries/decisions.rs`](../../crates/hyperform-engine/src/engine/queries/decisions.rs);
  the model card's status, download, cancel and remove are in
  `engine/queries/decision_model.rs` (Tauri `decision_model_status`,
  `download_decision_model`, `cancel_decision_model_download`,
  `remove_decision_model`; wording in
  `ui/src/lib/domain/settings/decisionModel.ts` and `nativeModel.ts`).
- Trust: `restrict_to_user_level` and `trust_gated` in
  [`settings/effective.rs`](../../crates/hyperform-engine/src/settings/effective.rs).

## The decision trace and the dataset

**In plain words.** Every decision leaves a receipt. The trace is a short
logbook kept in memory, with labels and numbers but none of your text; the
dataset is an opt-in notebook on disk, for measuring and training.

**How it works**
1. Each decision becomes a record: the feature and question, whether it ran
   in shadow, the provider, a short digest of the input (never the input
   itself), the model's answer and confidence, its latency or "cached",
   the fallback reason, what the engine did, why it overrode the model,
   and the tokens saved (or that would have been saved, in shadow). Your
   clicks on cards are records too, with provider `user`.
2. A session keeps its newest 500 records, totals for the whole session
   (decisions, shadow decisions, fallbacks, where abstaining by rules is
   not one, tokens saved and would-have-saved), and the median and
   95th-percentile latency of the kept records that were not cached. The
   trace lives in memory while the chat is open and survives settings
   reloads.
3. The Context tab's Decisions section shows it
   ([screens](features-desktop-screens.md#the-side-panel)).
4. With `record_dataset`, each answer is also appended as one JSON line to
   `<data dir>/decisions/dataset/<question>.jsonl`: the time, feature,
   shadow flag, the full input state and the answer. Unlike the trace it
   holds the input, which may include chat text; it never leaves the
   computer. Secret screening's questions are never recorded.

**For developers**
- [`trace.rs`](../../crates/hyperform-decisions/src/trace.rs)
  (`DecisionTrace`, `DecisionRecord`, `DecisionSummary`);
  `UseContext::record_of` and `record` in `decisions/context.rs`.
- `Engine::session_decisions` (`engine/queries/decisions.rs`, Tauri
  `session_decisions`) feeds `overlays/InspectorDecisions.svelte`, worded
  by `ui/src/lib/domain/decisionTrace.ts`, which reads only generic record
  fields, so a new use needs no GUI change.
- The dataset: `DecisionDataset` in
  [`store/src/dataset.rs`](../../crates/hyperform-store/src/dataset.rs),
  written by `DecisionService::ask` (`decisions/service.rs`).

See also: [Decision uses](features-decision-uses.md) ·
[Experimental features](../user-guide/15-experimental-features.md) ·
[Agents and context](features-agents-and-context.md) ·
[The desktop app's screens](features-desktop-screens.md) ·
[v2 engine architecture](../architecture/v2-engine.md) · [Crates](crates.md)
