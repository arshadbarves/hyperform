# Experimental features

New HyperForm features start as **experimental**: they are off until you
turn them on, and they stay experimental until they meet a measured bar
written down in advance. This page explains how to try one, the decision
model they use, where to see what they did, and how a feature becomes
standard.

This version includes 23 experimental features. Twenty-one of them are
*decision features*: each asks a small decision model a short question at
one moment of a turn and acts only on a confident answer. What each one
does is in [Decision features](16-decision-features.md). The other two are
[work plans](#work-plans), which keeps a chat's work as a plan of tasks,
and [parallel tasks](#parallel-tasks), which runs independent tasks of a
plan at the same time.

## Off, Shadow and On

Each experimental feature has its own switch with three positions, its
*mode*:

| Mode | What the feature does |
|---|---|
| **Off** (default) | Nothing. The app behaves as if the feature did not exist. |
| **Shadow** | It runs and records what it *would* have done, in the side panel's **Context** tab, but never changes a turn and shows nothing in the chat. |
| **On** | It acts, but only on answers the decision model is confident about; any other answer keeps today's behavior. |

Shadow is the safe way to try a feature: you can read what it would have
changed before you let it change anything. Every decision feature can run
in Shadow; work plans and parallel tasks have only Off and On.

## Turn a feature on

1. Open **Settings** (⌘, or Ctrl+,) and choose **Experimental**, in the
   **System** group between **Advanced** and **About & Updates**.
2. Choose the settings file under **Saving to** at the top right (**User**
   for every project, **This project**, or **Personal (local)**).
3. Pick **Off**, **Shadow** or **On** for the feature (**Off** or **On**
   for work plans and parallel tasks). Each feature shows its title, a
   one-line summary and an **Experimental** pill.
4. For a decision feature, set up the decision model on the **Decision
   model** card, which appears below the list as soon as one decision
   feature is Shadow or On ([The decision model](#the-decision-model)).

Open chats pick up the change at once. **Search settings** also finds
experimental features by name and by the words in their description.

### In a settings file

The `[experimental]` table holds one line per feature. The key is the
feature's id and the value is its mode, `"off"`, `"shadow"` or `"on"`
(`false` and `true` also work, as Off and On). The Experimental page
writes these lines for you; every decision feature's id is listed in
[Decision features](16-decision-features.md#all-features-at-a-glance),
work plans is `work_plans` and parallel tasks is `work_parallel`.

```toml
# ~/.config/hyperform/settings.toml
[experimental]
decisions_compaction = "shadow"
decisions_risk = "on"
work_plans = "on"
```

A line that can't apply is dropped with a warning on the Settings page and
as a notice when a chat opens:

| Line | What happens |
|---|---|
| A feature that has become standard | It is always on: "`experimental.<id>` is stable and always on; the setting is ignored". |
| An id HyperForm doesn't know (a typo) | Reported as an unknown key, `experimental.<id>`, and ignored. |
| `"shadow"` for a feature without Shadow (work plans) | It stays off: "`experimental.work_plans` does not support shadow mode; it stays off". |

## The decision model

The decision features rely on a **decision model**: a small, fast AI
model, separate from the one you chat with, that answers short
multiple-choice or yes/no questions about what is happening in a chat
(for example, whether an old tool result is still needed). HyperForm treats
its answer as advice. When the model is slow, missing or unsure, the
feature keeps today's behavior.

The model is Laya. HyperForm reaches it in one of two ways, the
**Runtime** choice at the top of the **Decision model** card:

| Runtime | `decisions.runtime` | How it works |
|---|---|---|
| **Sidecar** (default) | `"sidecar"` | A laya-serve (or Jev-compatible) server answers `POST /v1/systemone` requests on this computer. You run it yourself, or HyperForm starts it for you. |
| **In the app** | `"native"` | HyperForm runs the model itself from files you download on the card. No separate server. |

### Let HyperForm start it (sidecar)

Set the command that starts laya-serve. HyperForm then runs it for you as a
*sidecar*, a helper program that lives alongside the app:

```toml
[decisions.sidecar]
command = "laya-serve"   # a full path also works

[decisions]
checkpoint = "multilingual"   # the model laya-serve loads
```

- It starts once for the whole app, only when a decision feature runs or
  you click **Test connection**, and stops when you quit HyperForm.
- It listens on a free port on this computer only, with a new random key
  each time it starts. HyperForm tells it where through the environment
  variables `LAYA_HOST` (`127.0.0.1`), `LAYA_PORT`, `LAYA_API_KEY`,
  `LAYA_CHECKPOINT` and `LAYA_MAX_LEN`, and also names the checkpoint and
  input length in every request.
- It runs in the data folder with the same limited environment as the
  agent's commands, so use a full path if it isn't found
  ([Shell settings](12-settings-reference.md#shell)).
- When its command, `checkpoint` or `max_len` changes, the next start
  replaces the running sidecar. A sidecar that just started may need up
  to a minute to load its model.

When a sidecar command is set, `endpoint` is not used.

### Use a server you run yourself (endpoint)

```toml
[decisions]
endpoint = "http://127.0.0.1:8000"   # the default
api_key_env = "LAYA_API_KEY"         # only if your server needs a key
```

`api_key_env` is the *name* of an environment variable that holds the
key, not the key itself; HyperForm sends its value as
`Authorization: Bearer <key>`. If you name a variable that isn't set,
the decision model is not used. An address with a user name or password
in it is refused; use `api_key_env` instead.

### Run it in the app (native)

1. On the **Decision model** card, set **Runtime** to **In the app**.
2. Check **Checkpoint**: `multilingual` (the default, about 1.3 GB),
   `english` or `typed-decisions` (about 1.7 GB each).
3. In the **Native model** row below it, click **Download**. A ring and
   "Downloading *X* of *Y* (*P*%)" show the progress; **Cancel** stops it.
4. When the row says "Downloaded: *checkpoint* at *revision* (*size*)",
   click **Test connection**.

What to know:

- Downloads start only from this row, never by themselves. One model
  downloads at a time.
- The files come from Hugging Face (the `onnx-community` exports of Laya)
  at a pinned revision. Every file is checked against its pinned SHA-256
  checksum after the download and again when the model loads.
- A stopped or cancelled download keeps what it fetched: the row says
  "Paused at *X* of *Y*" and offers **Resume**, which continues from
  there.
- **Remove** asks "Remove the decision model?" and then deletes the
  model's folder. Cancel a running download first.
- The model loads in the background when a chat first needs it, which
  can take a few seconds; until then features keep today's behavior.
- Each checkpoint reads at most so many tokens per question, whatever
  `max_len` says: `english` 512, `multilingual` 8,192, `typed-decisions`
  1,024.

> **Note:** The in-app runtime is only in builds of HyperForm made with
> the native runtime (the `onnx` build feature), and the release
> installers don't include it yet. In a build without it, the row adds
> "This app was built without the native runtime, so it cannot run it;
> use the sidecar.", and the features fall back as described in
> [Troubleshooting](14-troubleshooting.md#the-decision-model-is-not-used).

### Test the connection

**Test connection** on the Decision model card asks the model one question
whose answer is known ("Does this text greet someone?" about a greeting),
starting the sidecar or loading the in-app model first. It uses the
settings in effect for the project open in Settings (your user settings
alone when none is open), waits up to 10 seconds, and tells you one of
these:

| Result | What to do |
|---|---|
| "Connected. Answered in N ms (P% confident)." | Nothing; it works. |
| "... That is slower than the N ms timeout, so features would fall back; raise the timeout or use a faster setup." | Raise `timeout_ms`, or use a faster machine or checkpoint. |
| "... It answered the test question wrongly, so check the checkpoint." | The model runs, but it is not the right one; check `checkpoint`. |
| "Not connected: *reason*." | Fix what the reason says ([Troubleshooting](14-troubleshooting.md#the-decision-model-is-not-used)). |
| "No answer yet: ... The model started moments ago and may still be loading; try again in a minute." | Wait a minute and test again. |

### Tune when features act

| Setting | Default | What it changes |
|---|---|---|
| `timeout_ms` | `250` | The longest wait for an answer (50 to 10,000 ms). A slower answer counts as no answer. |
| `threshold` | `0.8` | How sure an answer must be, from 0 to 1, before a feature acts on it. |
| `[decisions.calibration.<question>]` | – | Per-question corrections for a model that is too sure of itself: `temperature` (above 1 softens its confidence) and its own `threshold`. |

Some features have their own settings (for example how many files
prefetch may attach). Every key, with its range, is in the
[decision settings reference](17-decision-settings.md).

### Privacy

- Decision questions go only to the decision model. With the sidecar its
  address must be on this computer (`localhost`, `127.0.0.1` or `::1`)
  unless you turn on `allow_remote` (**Allow a remote endpoint**), which
  lets what the features ask about leave your computer. The in-app model
  never sends questions anywhere; only its download reaches the network.
- In a project you haven't trusted, the whole `[decisions]` table comes
  from your user settings only, because it decides where questions go,
  its sidecar command is a program, and its rules can ask you before tool
  calls ([workspace trust](03-permissions-and-safety.md#workspace-trust)).

## See what decisions did

The side panel's **Context** tab has a **Decisions** section, below
**Insights**, while a decision feature runs in the chat or decisions were
recorded. Unfold it to see:

- **Running**: the features running in this chat, with their mode;
- **Answered by**: the decision model, or "Rules only" when no model is
  connected (every feature then keeps today's behavior);
- **Speed**: how fast half of the answers, and 95% of them, came back;
- **Tokens**: tokens saved, and those that would have been saved in
  shadow;
- the 50 newest decisions, each as "Feature · question" with the answer
  and how confident it was, why it fell back (for example `timeout` or
  `low confidence`), what HyperForm did ("(shadow, nothing changed)" in
  shadow), tokens, and how long it took (or "cached"). "Overridden:"
  says why HyperForm set the model's answer aside. Your clicks on
  suggestion cards appear here too, answered by "user".

The section refreshes after each turn. It keeps no text from your chat,
only labels and numbers, and it starts empty each time a chat opens.

## Record decisions

To measure a feature, or to train or calibrate a model on your own work,
turn on **Record decisions** (`record_dataset`):

```toml
[decisions]
record_dataset = true
```

Each answer is then added as one line to
`decisions/dataset/<question>.jsonl` in the data folder
([where that is](13-sessions-and-data.md#where-things-live)): the time,
the feature, whether it ran in shadow, the question's full input and the
answer. Secret screening's questions are never recorded.

> **Warning:** Unlike the Decisions section, the dataset keeps each
> question's input, which can include text from your chats and your code.
> It stays on your computer; delete the folder to remove it.

## Work plans

**Work plans** (`work_plans`) keeps the work of a chat as a plan of tasks
that lasts beyond the conversation. Each task has acceptance criteria,
and a task counts as done only when its checks pass on the code as it is
now. The main agent can hand a task to a helper, and a plan audit points
out what the plan misses. You can pause a plan and pick it back up after
a restart or in another chat. It doesn't use the decision model. How to
use it: [Work plans](18-work-plans.md) and
[Pausing and continuing plans](19-pausing-and-continuing-plans.md).

## Parallel tasks

**Parallel tasks** (`work_parallel`) lets independent tasks of a plan you
approved run at the same time in helpers, each in its own worktree, and
brings their changes back one at a time. It needs work plans: on its own
it stays off with a warning. Only you allow parallel work, per plan. How
to use it: [Parallel tasks](18-work-plans.md#parallel-tasks).

## How features become standard

1. Each feature has a measured bar for leaving Experimental, such as
   "at least 15% fewer context tokens at equal task success". The bars
   and their results are recorded in the project's
   [status page](../status.md#graduation-criteria).
2. Once it meets its bar, the feature becomes **Stable**: it is always on,
   it leaves the Experimental page, and its `[experimental]` line is
   ignored with a warning.
3. The next release removes the old way of doing things and the
   feature's switch.

The changelog follows each step: the feature appears under **Added** as
experimental, then under **Changed** when it becomes standard, and the
old behavior under **Removed**. For decision features, today's rules stay
as the fallback whenever the model can't answer; only the switch goes
away.

See also: [Decision features](16-decision-features.md) ·
[Decision settings reference](17-decision-settings.md) ·
[Work plans](18-work-plans.md) ·
[Plan mode with work plans](05-plan-mode-questions-and-todos.md#plan-mode-with-work-plans) ·
[How work plans work](../how-it-works/features-work-plans.md) ·
[Sessions and data](13-sessions-and-data.md) ·
[Memory and context](07-memory-and-context.md#inspect-the-prompt) ·
[Troubleshooting](14-troubleshooting.md#the-decision-model-is-not-used)
