# Decision settings reference

The `[decisions]` table and its sub-tables configure the
[decision model](15-experimental-features.md#the-decision-model) that the
[decision features](16-decision-features.md) ask, and the few features
that have settings of their own. Whether each feature runs is set in
[`[experimental]`](12-settings-reference.md#experimental).

The **Decision model** card on **Settings → Experimental** edits the
`[decisions]` keys; it appears while a decision feature is Shadow or On.
The feature sub-tables and `[[decisions.rules]]` are edited in the
settings file. How settings files combine is explained in the
[settings reference](12-settings-reference.md#settings-files-and-layers).

> **Note:** In a project you haven't trusted, the whole `[decisions]`
> table, including its rules, comes from your user settings only
> ([workspace trust](03-permissions-and-safety.md#workspace-trust)).

## The decision model

`[decisions]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `runtime` | `"sidecar"`, `"native"` | `"sidecar"` | How the model is reached: a laya-serve server (**Sidecar**), or the model run inside the app from a downloaded copy (**In the app**). `native` needs a build with the native runtime and the model downloaded on the card; otherwise the features keep today's behavior with a notice. |
| `endpoint` | string | `"http://127.0.0.1:8000"` | A running laya-serve or Jev-compatible server; requests go to `<endpoint>/v1/systemone`. Trailing `/` removed; empty uses the default. Must be on this computer unless `allow_remote`. Not used when `sidecar.command` is set, or with `native`. |
| `api_key_env` | string | unset | Name of an environment variable holding the server's key, sent as `Authorization: Bearer`. If the variable isn't set, the model is not used. |
| `checkpoint` | string | `"multilingual"` | The Laya model to use: `multilingual`, `english` or `typed-decisions` (Laya's aliases such as `en` and `ml` also work). The sidecar loads it (`LAYA_CHECKPOINT`) and every request names it; with `native` it picks which files to download. Empty uses the default. |
| `max_len` | integer | `1024` | Tokens the model reads per question, 128–8192; longer inputs are cut. Sent with every request (and as `LAYA_MAX_LEN`). With `native`, each checkpoint also has its own cap: `english` 512, `multilingual` 8192, `typed-decisions` 1024. |
| `timeout_ms` | integer | `250` | Longest wait for an answer, 50–10000. A slower answer counts as no answer. |
| `threshold` | number | `0.8` | Confidence, 0 to 1, an answer needs before a feature acts on it. Out of range uses 0.8 with a warning. |
| `max_batch` | integer | `16` | Questions per request, 1–64; more are split into several requests. |
| `allow_remote` | bool | `false` | Allow an `endpoint` on another machine. While it is on, secret screening uses only its pattern detectors. |
| `record_dataset` | bool | `false` | Add each question's input and answer to `decisions/dataset/<question>.jsonl` in the data folder ([Record decisions](15-experimental-features.md#record-decisions)). |

On the card: **Runtime**, **Sidecar command**, **Decision model endpoint**,
**API key variable**, **Checkpoint**, the **Native model** row,
**Decision timeout**, **Confidence threshold**, **Input length**,
**Questions per request**, **Allow a remote endpoint**, **Record
decisions** and **Test connection**.

`[decisions.sidecar]` – `command` (string, unset): the shell command that
starts laya-serve. When set (with `runtime = "sidecar"`), it replaces
`endpoint`: HyperForm starts it once for the whole app, in the data folder,
on a free port on this computer, with `LAYA_HOST=127.0.0.1`, `LAYA_PORT`,
`LAYA_API_KEY` (new and random at each start), `LAYA_CHECKPOINT` and
`LAYA_MAX_LEN`, and stops it when you quit. It starts only when a decision
feature runs or on **Test connection**.

The in-app model is stored in `<data dir>/models/laya/<revision>/`
([Sessions and data](13-sessions-and-data.md#where-things-live)).

### Calibration

`[decisions.calibration.<question>]` – per-question corrections, where
`<question>` is a question's name as the Context tab's Decisions section
shows it, with `_` where it shows a space (for example
`compaction_relevant` or `risk_level`; a custom rule's is `rule_` plus its
name, such as `rule_prod`):

| Key | Type | Default | Meaning |
|---|---|---|---|
| `temperature` | number | `1.0` | Above 1 softens an over-confident question; 0 or less skips the entry with a warning. |
| `threshold` | number | unset | Confidence this question needs, 0 to 1; unset or out of range uses `decisions.threshold`. |

The decision benchmark prints fitted entries to paste here
([status](../status.md#decision-layer-tests-and-benchmarks)).

## Feature settings

| Table and key | Type | Default | Range | Feature |
|---|---|---|---|---|
| `[decisions.prefetch]` `max_files` | integer | `3` | 1–10 | [File prefetch](16-decision-features.md#file-prefetch): most files attached to one message. |
| `[decisions.prefetch]` `max_tokens` | integer | `6000` | 500–50000 | File prefetch: estimated tokens all attached files may take together; a file that doesn't fit is skipped. |
| `[decisions.routing]` `allow_model_switch` | bool | `false` | – | [Per-task routing](16-decision-features.md#per-task-routing): also choose between `model.main` and `model.fast`, for subagents whose model is `inherit` too. Off: only the effort is chosen. |
| `[decisions.loop_guard]` `max_reminders` | integer | `3` | 1–10 | [Loop guard](16-decision-features.md#loop-guard): reminders per turn before it asks you. |
| `[decisions.task_view]` `keep_recent_exchanges` | integer | `2` | 1–20 | [Task-scoped history](16-decision-features.md#task-scoped-history): newest exchanges that always stay in view. |

Out-of-range prefetch values are clamped with a warning; the loop guard
and task view values are held to their range where they are used.

## Custom rules

`[[decisions.rules]]` – one table per rule, used by
[Custom decision rules](16-decision-features.md#custom-decision-rules)
(`decisions_custom_rules`). Rules from your user settings and the
project's add up; the project's apply only once it is trusted.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | unset | Shown in approvals and notices; the question when unset. |
| `event` | `"PreToolUse"`, `"UserPromptSubmit"` | – (required) | Before an allowed tool call, or about the message that opens a turn. |
| `matcher` | string | unset (every tool) | `PreToolUse` only: a regular expression over tool names, matched against the whole name as in [hooks](08-hooks.md); `*` also means every tool. |
| `question` | string | – (required) | What to ask about the call or the message. |
| `options` | list of strings | `[]` (yes or no) | Otherwise 2 to 20 different answers to choose from. |
| `when` | list of strings | `["yes"]`, or the first option | The answers that trigger `action`; each must be one of the answers. |
| `action` | `"ask"`, `"notice"`, `"remind"` | – (required) | What a triggering answer does (below). |

| Action | `PreToolUse` | `UserPromptSubmit` |
|---|---|---|
| `ask` | An approval card before the call ("Decision rule: …"); in Bypass mode a notice instead. | The agent is told to ask you before it changes anything. |
| `notice` | A notice ("Decision rule on *Tool*: …"). | A notice ("Decision rule on your message: …"). |
| `remind` | A note for the agent after the call runs. | A note for the agent at the start of the turn. |

A rule never allows anything, and your message always reaches the agent.

```toml
[[decisions.rules]]
name = "area"
event = "UserPromptSubmit"
question = "Which part of the app does this message ask to change?"
options = ["ui", "database", "docs"]
when = ["database"]
action = "remind"
```

A rule that can't run is skipped with a warning naming its position, such
as "decisions.rules #2: action `allow` is not ask, notice or remind; rule
skipped". The checks: `event` is one of the two events, `question` is not
empty, `action` is one of the three, `options` has 0 or 2 to 20 answers
with no repeats, every `when` answer is an option, and `matcher` is a
valid regular expression on a `PreToolUse` rule.

## Example

```toml
[experimental]
decisions_compaction = "shadow"
decisions_risk = "on"

[decisions]
runtime = "sidecar"
timeout_ms = 300
threshold = 0.85

[decisions.sidecar]
command = "laya-serve"

[decisions.loop_guard]
max_reminders = 2
```

See also: [Experimental features](15-experimental-features.md) ·
[Decision features](16-decision-features.md) ·
[Settings reference](12-settings-reference.md) ·
[Troubleshooting](14-troubleshooting.md#the-decision-model-is-not-used)
