# Decision uses

What each of the 21 experimental decision features does inside the
engine, grouped the way Settings groups them. The layer they share (the
service, the [seams](features-experimental-and-decisions.md#seams-where-a-decision-may-act),
dispatch, the model and the trace) is in
[Experimental features and the decision layer](features-experimental-and-decisions.md).
What you see when each one is On is in
[Decision features](../user-guide/16-decision-features.md). Terms are in
the [glossary](glossary.md).

Every use follows the same contract: it asks only at its seams, acts only
in On mode and only on confident answers, and does exactly what Off does
when the model is missing, slow, unsure or wrong-shaped.

## Context and cost

**In plain words.** These uses decide what the model reads, like an
assistant who tidies the desk before each meeting: old papers that no
longer matter go in a drawer (with a note saying which drawer), and the
files you will need are already on the table.

**How it works**
1. **Compaction** (Pressure seam): when context pressure plans to clear
   old tool results, each candidate is asked about against your latest
   request and open todos. A confident "not needed" clears it even when it
   is short (from a quarter of the usual size floor); a confident "needed"
   keeps it. Results about files your request names, the latest failing
   check and files edited after it are kept whatever the model says. A
   cleared result the agent reads again (After-call seam) is traced as a
   *reread*, a sign the clear was wrong.
2. **Task view** (Turn start, then applied when the request is saved):
   see [the next section](#task-scoped-history-and-its-record).
3. **First-request context** (Turn start, first request only): project
   files and deferred MCP tools that share words with the request are
   asked about, one yes/no each. Relevant files rank right after the focus
   files in a rebuilt
   [repository map](../user-guide/07-memory-and-context.md#the-repository-map),
   which then stays byte-stable for the chat; relevant tools are loaded up front
   instead of after a `LoadMcpTools` round.
4. **Prefetch** (Turn start): for a request of at least four words, up to
   eight candidates (files it names, files defining a symbol it mentions,
   and on a first turn files changed in the working tree) are asked about.
   Confident picks are attached to the opening message like user
   attachments, within `[decisions.prefetch]` `max_files` and
   `max_tokens`, and only when Read would be allowed without asking. At
   turn end it records how many tool rounds the main agent ran before its
   first edit, and how many files were prefetched (in Shadow, the baseline
   to compare On against).
5. **Output trim** and **search rank** (Relevance seam): a tool about to
   cut a long result asks which parts matter. Bash and background-job
   output is split into 40-line windows; the head, the tail and every
   window not confidently ruled out stay, with a marker for each gap, and
   the full output is still saved. Grep and Glob show the useful results
   first so they survive the cut. No answer keeps today's cut.

**For developers**
- Uses in [`decisions/uses/`](../../crates/z-engine-engine/src/decisions/uses/):
  `compaction/` (`advise.rs`, `reread.rs`, `memory.rs`),
  `session_context/` (`candidates.rs`, `first_request.rs`, `preload.rs`),
  `prefetch/` (`candidates.rs`, `pick.rs`, `attach.rs`, `metrics.rs`),
  `output_trim.rs`, `search_rank.rs` and their shared `relevance.rs`.
  `digest.rs` builds the small digests sent as state.
- Pure planning in [`z-engine-context`](crates.md#z-engine-context):
  `compaction/ranked.rs` (relevance-aware microcompaction), `keeps.rs`
  (hard keeps), `touch.rs` (what a call touched, the files a request
  names); the seeded map ranking in `repo_map/`.
- Tools ask through the `RankRequest` port in
  `z-engine-tools/src/ports/relevance.rs`, served by the engine's
  `ports/relevance.rs` (`rank_items`).

## Task-scoped history and its record

**In plain words.** When you switch to a new task in a long chat, the
model stops rereading the old one. It is like closing yesterday's folders
but keeping a list of them on the desk: anything can be pulled back out,
and your own copy of the conversation never loses a page.

**How it works**
1. At turn start the use cuts the history into whole exchanges (a user
   turn and everything up to the next one; a tool call never leaves its
   result). It asks whether the new message starts another task; more
   than five minutes idle, or right after a summary, also counts.
2. On a new task it asks, per earlier exchange, whether the new task needs
   it. Hard keeps stay whatever the model says: the newest exchanges
   (`keep_recent_exchanges`), exchanges about files the request names, a
   still-failing check, the latest todo list while todos are open, and the
   latest summary. An exchange that set a standing rule keeps its opening
   message.
3. A cost check guards the prompt cache: while the cache is warm (5
   minutes) it acts only on at least 30,000 tokens of history and only
   when it sets aside at least 40% of it.
4. Once your message is saved, each set-aside exchange is written whole to
   the chat's artifacts (`task-view-turn-N.txt`) and one index message
   lists them with the file to read. The working set becomes the kept
   messages plus the index; the transcript is untouched. Anything
   unexpected (the history changed, a write failed) leaves it whole.
5. The session log gets a `TaskView` record with the boundary message, the
   number set aside, the tokens, and the kept message ids in order (plus
   the index message). Replay and reopening rebuild the same working set
   from it, and the GUI draws a divider per record (`TaskViewApplied`,
   `SessionSnapshot.task_views`).
6. **Include full history** (`Command::IncludeFullHistory`) puts the full
   history back until the next task, journals a `TaskView` record with
   `restored` set and no ids, and traces the click with provider `user`.

**For developers**
- [`decisions/uses/task_view/`](../../crates/z-engine-engine/src/decisions/uses/task_view/):
  `plan.rs` (the use), `ask.rs`, `history.rs`, `readback.rs`,
  `memory.rs` (`TaskViewMemory`: the pending plan, the cache clock) and
  `apply.rs` (`apply_task_view_for`, called from `session/turn.rs`;
  `include_full_history`, from `session/actor.rs`).
- Pure planning: `compaction/exchanges.rs` and `compaction/task_view.rs`
  in `z-engine-context`.
- The record: `LogRecord::TaskView { view, working, index }` in
  [`store/src/record.rs`](../../crates/z-engine-store/src/record.rs),
  replayed in `store/src/replay.rs`; `TaskViewInfo` in
  `z-engine-protocol` (`decisions`).
- Tested end to end in `tests/decisions_task_view.rs`.

## Per-task routing

**In plain words.** A quick question doesn't need the same effort as a
redesign. Routing sizes each new task once and keeps that choice for the
whole task, like picking the right tool before starting rather than
switching mid-job.

**How it works**
1. At a new task (the first turn, more than five minutes idle, or a
   confident "new task"; short replies count as follow-ups) the model
   rates the request simple, moderate or complex: low, medium or high
   effort, only while your effort is on **auto**. Large requests are never
   rated down to simple.
2. With `[decisions.routing] allow_model_switch`, a simple task may run on
   the fast model when the chat uses the main model, nothing picked a
   model for the turn, and the fast model's context window is at least as
   large.
3. The route holds for every follow-up, so the prompt cache survives. A
   subagent whose model is `inherit` is routed once at launch, for the
   model only.
4. Each applied route emits `RouteChosen`, which the GUI shows as a chip.

**For developers**
- [`decisions/uses/routing/`](../../crates/z-engine-engine/src/decisions/uses/routing/):
  `decide.rs`, `signals.rs` (size, follow-up), `memory.rs`
  (`RouteMemory`), `start.rs` (`route_new_task`); the seam is
  `seams/route.rs`, called from `session/turn.rs` and
  `orchestration/launch.rs`.

## Safety

**In plain words.** These uses are extra lookouts. They can raise a hand
(an approval card, a notice, a note for the agent) but never open a door
the permission rules keep closed, nor close one you opened.

**How it works**
1. **Risk review**: at the Tool gate, an allowed call that isn't
   read-only is asked about (intent, risk level, injection); a confident
   bad answer turns Allow into Ask. After WebFetch, WebSearch and MCP
   calls, a result that tries to instruct the agent gets a warning note.
2. **Secret screening** (Before a request): detectors flag well-known key
   formats, private keys, JWTs, passwords in URLs and high-entropy values
   assigned to secret-like names; the model judges up to eight other
   assignment lines, only while its endpoint is local. One approval card
   lists the kinds; Deny or no answer replaces each value with a
   placeholder in this and every later request (a per-chat ledger). The
   values are never traced or recorded.
3. **Custom rules**: your `[[decisions.rules]]`, asked at Turn start
   (`UserPromptSubmit`) or for tool calls (`PreToolUse`, by matcher). An
   answer listed in `when` asks you, posts a notice, or adds a reminder.
   `ask` and `notice` rules on a call are asked at the Tool gate;
   `remind` rules after the call ran, so the note reaches the agent that
   made it.
4. **Loop guard**: per agent and turn, it notices the same call and result
   three times, or the same failure after an edit, without the model; for
   other failure runs it asks "is the agent making progress?" at most every
   four calls. Each finding reminds the agent with advice for that kind of
   failure and posts a notice; after `max_reminders` the next allowed call
   asks you.
5. In Bypass mode each of them posts a notice instead of asking.

**For developers**
- `uses/risk/` (`gate.rs`, `injection.rs`), `uses/secret_screen/`
  (`detect.rs`, `screen.rs`, `ledger.rs`; the seam is `seams/request.rs`,
  called from `run/agent.rs`), `uses/custom_rules/` (`judge.rs`,
  `prompt.rs`, `tool.rs`; rules validated in `z-engine-config`
  `settings/decisions_rules.rs`) and `uses/loop_guard/` (`detect.rs`,
  `steps.rs`, `watch.rs`, `guard.rs`).
- Escalation is `escalate` in `seams/tool_gate.rs`: only an Allow becomes
  an Ask, with no rule to save.
- End-to-end tests: `tests/decisions_secret_screen.rs`,
  `tests/decisions_loop_guard.rs`.

## Verification

**In plain words.** These uses keep verification honest and quick: one
notices when the agent says "done" without proof, the other skips checks
that can't be affected.

**How it works**
1. **Completion check** (Completion seam): when the main agent stopped
   with changes no check backs, four questions about the final message and
   what ran this turn decide whether it claims success. The verifier then
   runs the test, build and typecheck checks (auto and strict, when none of
   the configured ones matched) or shows the claim on the receipt
   (`CompletionClaimUnchecked`). A claim never changes the badge.
2. **Check selection** (Check select seam): checks whose ecosystem owns a
   changed file always run; the model is asked about the rest, and a check
   is skipped only on a confident "no" for every one asked. Nothing is
   skipped when a test file changed, and strict keeps a test, build or
   typecheck check.

**For developers**
- `uses/completion_check.rs` (seam called from `verify/claims.rs`) and
  `uses/check_select.rs` (from `verify/modes.rs`); the path mapping and
  `skip_checks` are in `z-engine-verify/src/narrow.rs`.

## Suggestions, guidance and the app

**In plain words.** These uses make small, polite offers: a card you can
click, a short note the agent may follow, or a hint to the pet and the
Inbox. None of them does anything on its own.

**How it works**
1. Cards (`Suggested`, answered by `Command::ResolveSuggestion`): **plan
   suggestion** (Turn start, Default mode, a confident "large"), **standing
   rule** (Turn start, a sentence that sets a rule for the future, saved
   through the Memory tab's write path) and **review suggestion** (Stop
   seam; path rules first, the model only for ambiguous files). Each click
   is traced with provider `user`; after a dismissal the plan and review
   cards stop for the chat. Shadow shows no card.
2. Notes for the agent at Turn start: **correction signal** (your message
   corrects the agent or sounds frustrated) and **hints** (up to two
   confident fits among the six skills and agents sharing the most words
   with the request). Each hint is given once per chat.
3. **Repeated-question check** (Ask user seam): when every question has a
   confident earlier answer in your recent messages, the agent gets a
   pointer to them instead; the same question asked again reaches you.
4. **Pet mood** (Turn end, in the background): the tone of a turn you
   didn't stop, sent as `TurnToneJudged`; the pet uses it as one more
   input.
5. **Inbox priority** (Attention seam): approvals, questions, plan reviews
   and warning or error notices get low, normal or high (`UrgencyScored`).
   The Inbox sorts by it, and `Notification` hooks fire only when
   something is high or unrated. There are no system notifications.

**For developers**
- `uses/plan_suggest.rs`, `memory_suggest.rs`, `review_suggest/`
  (`rules.rs`, `suggest.rs`), `user_signal.rs`, `hints.rs`,
  `question_check.rs`, `pet_mood.rs` and `inbox_priority.rs`, with the
  shared prefilters in `guidance.rs` and `decisions/memo.rs` (`UseMemo`,
  so nothing is offered twice). Cards go through `decisions/suggest.rs`.
- The GUI: `planning/Suggestions.svelte` and its three cards,
  `chat/RouteChip.svelte`, `chat/TaskViewDivider.svelte`, the claim badge
  in `chat/TurnFooter.svelte`; the tone in
  `lib/domain/pet/decisionMood.ts` and the urgency in
  `lib/domain/inbox.ts`.

## Testing the uses

**In plain words.** Each use is tested to do nothing harmful when the
model misbehaves, and the whole set is tested together.

**How it works**
1. Each use's own tests run against a scripted model that answers by
   question name, and check that Off and Shadow change nothing.
2. End-to-end tests in `crates/z-engine-engine/tests/`: every feature On
   with a model that always gives the acting answer, where every safety
   rule still holds (`decisions_all_on.rs`); every feature On with a dead
   or hanging model matching Off (`decisions_fallback.rs`); every feature
   in Shadow matching Off while the trace records what each would have
   done (`decisions_shadow.rs`); plus one file each for compaction, task
   view, first-request context, loop guard, secret screening and the
   repeated-question check.
3. A live A/B test and an offline benchmark need a real model and are
   ignored by default; how to run them is in
   [status](../status.md#decision-layer-tests-and-benchmarks).

**For developers**
- `decisions/uses/scripted.rs` (the scripted provider) and
  `decisions/seams/tests.rs` (the Off guarantee per seam).

See also: [Experimental features and the decision layer](features-experimental-and-decisions.md) ·
[Decision features](../user-guide/16-decision-features.md) ·
[Decision settings reference](../user-guide/17-decision-settings.md) ·
[Crates](crates.md)
