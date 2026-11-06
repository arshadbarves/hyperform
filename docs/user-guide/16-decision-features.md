# Decision features

Each experimental feature in this version asks the
[decision model](15-experimental-features.md#the-decision-model) a short
question at one moment of a turn, such as "is this old tool result still
needed?" or "does this command look risky?". This page lists what each
one does, what you see when it is **On**, and its settings. To turn one
on, see [Experimental features](15-experimental-features.md#turn-a-feature-on).

Three rules hold for every feature:

- **Shadow** records what the feature would have done in the Context
  tab's **Decisions** section and changes nothing: no card, chip, notice
  or reminder.
- **On** acts only on answers the model is confident about. When the
  model is missing, slow, unsure or wrong-shaped, the feature does exactly
  what Off does.
- A feature can add an approval, a notice, a note for the agent or a card
  for you. It can never allow a call the permission rules would ask about,
  deny one, cancel a turn, or hide content from you.

A few parts work without the model by design, so they act even when it is
down: the loop guard's repeat detection, secret screening's pattern
detectors and review suggestion's path rules.

## All features at a glance

| Feature | `[experimental]` id | What you notice when it is On |
|---|---|---|
| [Relevance-aware compaction](#relevance-aware-compaction) | `decisions_compaction` | Nothing in the chat; fewer tokens per request |
| [Task-scoped history](#task-scoped-history) | `decisions_task_view` | A divider "N earlier exchanges … set aside for this task" |
| [First-request context](#first-request-context) | `decisions_session_context` | Nothing in the chat |
| [Per-task routing](#per-task-routing) | `decisions_routing` | A **Routed** chip in the transcript |
| [Loop guard](#loop-guard) | `decisions_loop_guard` | "Loop guard: …" notices, then an approval card |
| [Completion check](#completion-check) | `decisions_completion_check` | **Claimed, not checked** on the turn receipt |
| [Risk review](#risk-review) | `decisions_risk` | "Risk review: …" approval cards |
| [Relevant output trimming](#relevant-output-trimming) | `decisions_output_trim` | "[... lines A-B left out …]" in long command output |
| [Search ranking](#search-ranking) | `decisions_search_rank` | Relevant matches first when a search is cut |
| [File prefetch](#file-prefetch) | `decisions_prefetch` | Nothing in the chat; fewer reads |
| [Plan suggestion](#plan-suggestion) | `decisions_plan_suggest` | A "Plan first?" card |
| [Correction signal](#correction-signal) | `decisions_user_signal` | Nothing in the chat |
| [Skill and agent hints](#skill-and-agent-hints) | `decisions_hints` | Nothing in the chat |
| [Standing-rule suggestions](#standing-rule-suggestions) | `decisions_memory_suggest` | A "Remember this for next time?" card |
| [Repeated-question check](#repeated-question-check) | `decisions_question_check` | Fewer repeated questions |
| [Review suggestion](#review-suggestion) | `decisions_review_suggest` | A "Run a review?" card |
| [Pet mood from turn tone](#pet-mood-from-turn-tone) | `decisions_pet_mood` | The pet's face after a turn |
| [Inbox priority](#inbox-priority) | `decisions_inbox_priority` | The Inbox's order; fewer `Notification` hooks |
| [Check selection](#check-selection) | `decisions_check_select` | Fewer checks run in auto and strict verification |
| [Custom decision rules](#custom-decision-rules) | `decisions_custom_rules` | Your rules' approvals, notices and notes |
| [Secret screening](#secret-screening) | `decisions_secret_screen` | A "Send … possible secrets …?" approval card |

## Context and cost

These features keep requests smaller, so turns cost less and the model
sees what matters.

### Relevance-aware compaction

When older tool results are cleared to free context
([automatic compaction](07-memory-and-context.md#automatic-compaction)),
the model judges each one against your latest request and open todos.
Results it calls unrelated are cleared even when they are short (from 250
characters instead of the usual 1,000); results it calls needed stay.
Results about files your request names, the latest failing check, files
edited since, and the most recent results are always kept. Until the model has given one confident answer, clearing works as
with the feature Off. A cleared result the agent reads again is recorded
as a *reread* in the Decisions section.

### Task-scoped history

When you start a new task in a long chat, earlier exchanges the new task
doesn't need are set aside: the model gets a one-line index of them
instead, saying which file holds each one's full text, and can read it
back. Your transcript keeps everything.

- A new task is the model's answer to "does this message start another
  task?", or any message after more than five minutes idle or right after
  a summary.
- Always kept: the newest exchanges (`keep_recent_exchanges`, default 2),
  exchanges about files the new request names, a check that is still
  failing, the latest todo list while todos are open, and the latest
  summary. An earlier message that set a rule for the whole chat keeps its
  opening message in view.
- Rebuilding the request costs a prompt-cache miss, so while the cache is
  still warm it acts only on at least 30,000 tokens of history, and only
  when it sets aside at least 40% of it.
- A divider in the transcript reads "N earlier exchanges (X tokens) set
  aside for this task". Click **Include full history** to bring them back
  until the next task; the divider then reads "Full history included for
  this task". Dividers survive closing and reopening the chat.

### First-request context

On a chat's first request only, files that look relevant to it are listed
first in the [repository map](07-memory-and-context.md#the-repository-map),
and deferred MCP tools that fit the request are loaded up front instead of
after a separate loading step. From then on the map stays the same for
the whole chat, so prompt caching keeps working.

### File prefetch

When your message has at least four words, HyperForm gathers up to eight
candidate files (files the message names, files that define something it
mentions, and on a chat's first turn files changed in the working tree)
and asks about each one. The confident picks are attached to your message
the way attached files are, so the agent reads less. A file is attached
only if it is text, inside the project, not read yet, at most 64 KB, and
allowed for Read without asking. Settings: `[decisions.prefetch]`
`max_files` (default 3) and `max_tokens` (default 6,000).

### Relevant output trimming

Long output of Bash and background shell jobs (not helper agents) is cut
into 40-line windows. Output shorter than four windows is never trimmed.
Up to 24 middle windows are asked about (the most error-like ones when
there are more); the first and last windows and every window not
confidently ruled out stay, in order. A gap reads
`[... lines A-B left out: not relevant to the current task.]`, and the
first one says where the full output is saved. Without an answer in time,
the usual head and tail cut applies.

### Search ranking

When Grep or Glob finds more results than it can show, up to 24 results
are asked about and the useful ones are shown first so they survive the
cut. Glob looks at up to three times its limit; Grep groups matches by
file and saves the full result to a file. Below the cap, and for a Grep
with `head_limit` or `offset`, nothing changes.

### Per-task routing

When a new task starts, the model rates the request as simple, moderate
or complex, which sets the reasoning effort to low, medium or high, but
only while you leave effort unset. The choice holds for every follow-up
in the task, so the prompt cache keeps working.

- A new task is the chat's first turn, a turn after more than five
  minutes idle, or a "yes" to "is this a new task?". Replies of 24
  characters or fewer count as follow-ups.
- A large request (over 1,500 characters, or naming four or more files)
  is never routed down to simple.
- With `[decisions.routing] allow_model_switch = true`, a simple task
  runs on your fast model instead of the main one, when the chat uses the
  main model, no slash command picked a model for the turn, and the fast
  model's context window is at least as large. Subagents whose model is `inherit`
  are routed the same way, for the model only.
- A chip in the transcript shows **Routed** (or **Subagent routed**), the
  effort or model, and why. Chips are not kept when you reopen the chat.

See [Models, providers and cost](11-models-providers-and-cost.md#reasoning-effort).

## Safety

### Risk review

Before a call your rules already allow, and that isn't read-only, the
model is asked whether it serves your request and how risky it is. A call
that looks harmful, driven by instructions found in a file or page, in
need of your approval, or off task gets an approval card that reads
"Risk review: this *Tool* call …". In Bypass mode it runs anyway with a
warning notice. Results of WebFetch, WebSearch and MCP tools that look
like instructions aimed at the agent get a warning note in front of them
for the agent; nothing is removed.

### Secret screening

Before each request to your provider, new tool results are checked for
likely credentials: AWS, GitHub, Slack, OpenAI, Anthropic, Stripe and
Google keys, private key blocks, JWTs, passwords in URLs, and long random
values assigned to names like `token` or `password`. The model judges up
to eight other assignment lines, but only while it runs on this computer
(`allow_remote` off). If anything is found, an approval card asks "Send
*N* possible secrets from *Tool* output to the model?" and lists only
each one's kind, tool and length.

- **Deny** (or stopping the turn) replaces each value with
  `[secret withheld by the user: <kind>]` in this and every later
  request. **Allow** sends it, and the same value isn't asked about again
  in the chat. The card has no "always allow".
- In Bypass mode the values are sent with a warning notice.
- Only the main conversation is screened; titles, summaries and other
  side requests to the fast model are not.

### Custom decision rules

Your own questions, asked before tool calls (`PreToolUse`) or about your
message (`UserPromptSubmit`). When the answer is one you listed, the rule
asks you (an approval before the call, or the agent asks you before
changing anything), posts a notice, or adds a note for the agent.

```toml
[[decisions.rules]]
name = "prod"
event = "PreToolUse"
matcher = "Bash"
question = "Does this command touch production?"
action = "ask"
```

A rule never allows anything; in Bypass mode an `ask` becomes a notice.
Rules in your user settings and the project's add up, but a project's
rules apply only once you trust it. Every field is in the
[decision settings reference](17-decision-settings.md#custom-rules).

### Loop guard

The guard watches each agent's calls within a turn. Without asking the
model, it notices the same call with the same result three times (again
at six, nine…) and the same failure coming back after an edit. For less
clear runs of failures it asks the model "is the agent making progress?",
at most every four calls. Each time it reminds the agent with advice that
fits the failure (retry, fix the code, or fix the environment) and posts
a notice: "Loop guard: … Reminded the agent (n of max)." After
`max_reminders` reminders in a turn (default 3), the agent's next allowed
call shows an approval card: **Allow** lets it run, **Deny** with a note
tells the agent what to do instead. In Bypass mode there is only a
notice. It never switches models or stops the turn.

## Verification

### Completion check

When a turn changed files and ended **Unverified**, the model reads the
agent's final message. If it confidently claims the work is done or that
tests passed:

- in `report` mode, the receipt shows **Claimed, not checked** (hover for
  why);
- in `auto` and `strict` mode, when none of your auto checks matched, the
  project's test, build and typecheck checks run the usual way; if
  nothing can run (or the project isn't trusted), the badge shows.

The claim by itself never makes a turn Verified, and recorded checks are
never changed. The badge is not kept when you reopen the chat. See
[Verification](10-verification.md).

### Check selection

In `auto` and `strict` verification, checks for the language of a changed
file always run (for example cargo checks for `.rs` files, npm checks for
JavaScript and TypeScript). The model is asked about the other checks, up
to eight; a check is skipped only if every answer is a confident "no".
Every check runs when a test file changed, and `strict` keeps at least one
test, build or typecheck check.

## Suggestions and guidance

Cards appear at the end of the transcript and act only when you click.
They hide once the next turn starts and are not kept when you reopen the
chat.

### Plan suggestion

In Ask mode with plan mode off, when your message has at least eight words
and the model calls it a large change, a card asks "This looks like a large
change. Plan first?". **Plan first** turns on [plan mode](05-plan-mode-questions-and-todos.md);
**Not now** dismisses it, and the chat offers no more.

### Standing-rule suggestions

When a sentence of your message sets a rule for the future ("always use
pnpm", "never touch the generated folder"), a card asks "Remember this for
next time?" with **Save to AGENTS.md** (shared with the project),
**Save just for me** (`AGENTS.local.md`) and **Not now**. Saving appends
the rule as a list item through the Memory tab's write path. A rule
already in the file isn't offered.

### Review suggestion

When a turn stops, its changed files are checked: path rules spot
authentication code, database migrations, CI configuration and files
deleted by shell commands, and the model is asked about other sensitive
looking files. A card then says "These changes touch … Run a review?";
**Run review** runs [`/review`](06-commands-and-skills.md) once the agent
is idle. Each file is suggested once, and **Not now** ends suggestions for
the chat.

### Correction signal

When your message seems to correct the agent or sounds frustrated (read
next to the agent's last sentence; never on a chat's first message), the
agent gets a note to restate its plan and to ask with a question before
large edits. Permissions don't change.

### Skill and agent hints

When your message has at least five words, the installed skills and
custom agents that share the most words with it (up to six) are each
asked about. Up to two confident fits are named to the agent in a note;
it still decides whether to use them. Each is named at most once a chat.

### Repeated-question check

Before a question from the agent reaches you, the model checks whether
your earlier messages already answered every part of it. If so, the agent
gets a pointer to those messages instead. If it asks the same question
again, the question reaches you.

## The app

### Pet mood from turn tone

When a turn ends (not one you stopped), the model judges it as smooth,
struggling, blocked or done well. The [pet](02-everyday-use.md#your-pet)
uses that only after the turn is over and never against the outcome:
done well looks proud, smooth happy or content, struggling relieved or
worried, blocked confused or worried. Without an answer its usual mood
applies.

### Inbox priority

Approvals, questions, plans to review, and warning or error notices are
rated low, normal or high. The [Inbox](02-everyday-use.md#the-inbox)
sorts **Needs you** items by urgency within each group (unrated counts as
normal). Your [`Notification` hooks](08-hooks.md) then fire only when
something waiting is high, or couldn't be rated. HyperForm itself sends no
system notifications.

## Known limits

- Route chips, suggestion cards and the **Claimed, not checked** badge
  live only while the chat is open.
- Secret screening covers the main conversation only.
- The in-app runtime is not the default, and the release installers are
  built without it ([Run it in the app](15-experimental-features.md#run-it-in-the-app-native)).
- The in-app model files are community ONNX exports of Laya
  (`onnx-community/laya*-ONNX`) at a pinned revision, not files published
  by Laya's authors; the repository's `NOTICE` file covers attribution.

See also: [Experimental features](15-experimental-features.md) ·
[Decision settings reference](17-decision-settings.md) ·
[Permissions and safety](03-permissions-and-safety.md) ·
[Memory and context](07-memory-and-context.md) ·
[Verification](10-verification.md)
