# Glossary: experimental features

Part of [How HyperForm works](README.md). Short, plain-language definitions
of the terms used for experimental features, the decision layer and work
plans, in alphabetical order. Every other term is in the main
[glossary](glossary.md).

### Acceptance criterion

One condition a task in a [work plan](#work-plan) must meet to be done, and
how it is shown: a [check](glossary.md#check) of a kind (such as `test`),
an inspection the agent confirms, or a check you make yourself. See
[the completion gate](features-work-plans.md#tasks-criteria-and-the-completion-gate).

### Audit finding

One thing a [work plan](#work-plan) misses, such as a requirement with no
task or a risky path without a test, with a severity (high, medium or
low) and often a suggested fix. HyperForm's own checks and the read-only
`audit` agent raise them; you **Add to plan** or **Dismiss** each. See
[The plan audit](features-work-parallel.md#the-plan-audit).

### Completion gate

The rule that a task is marked done only when every
[acceptance criterion](#acceptance-criterion) holds or was waived (a check
criterion needs a passing check on the current code, by
[fingerprint](glossary.md#fingerprint)), its subtasks are closed and its
dependencies are done. See
[the completion gate](features-work-plans.md#tasks-criteria-and-the-completion-gate).

### Decision model

A small, fast model on your computer (a server, a [sidecar](#sidecar) or
the [native runtime](#native-runtime)) that experimental decision features
ask short questions. Its answer counts only when it is confident. See
[The decision layer](features-experimental-and-decisions.md#the-decision-layer).

### Decision use

The code behind one decision feature: it asks the
[decision model](#decision-model) at a [seam](#seam) and may act on a
confident answer. This version has 21. See
[Decision uses](features-decision-uses.md).

### Driving chat

The one chat that works a [work plan](#work-plan) at a time: the chat that
attached it last. Attaching the plan in another chat moves it there, and
the old chat's helpers on it stop. See
[One chat drives a plan](features-work-resume.md#one-chat-drives-a-plan).

### Experimental feature

A new feature that stays optional, and off unless you turn it on in
**Settings → Experimental**, until it meets a measured bar and
[graduates](#graduation).
See [Experimental features and graduation](features-experimental-and-decisions.md#experimental-features-and-graduation).

### Fallback (decision)

When the [decision model](#decision-model) is unset, fails, is slow or is
unsure, the feature does exactly what it does when Off. See
[The decision layer](features-experimental-and-decisions.md#the-decision-layer).

### Feature flag

An [experimental feature](#experimental-feature)'s mode in
`[experimental]`: Off (the default), [Shadow](#shadow-mode) or On. See
[Experimental features and graduation](features-experimental-and-decisions.md#experimental-features-and-graduation).

### Graduation

An [experimental feature](#experimental-feature) that met its bar becomes
Stable and always on; the next release removes the old behavior and its
flag. See [Experimental features and graduation](features-experimental-and-decisions.md#experimental-features-and-graduation).

### Held task

A task of a [work plan](#work-plan) that may not start for now: you put it
on hold (**Hold**, until **Release**), or [reconciliation](#reconciliation)
found a problem it is part of, and it waits until you accept the plan
change. Other tasks keep running. See
[Reconciliation and /replan](features-work-plans.md#reconciliation-and-replan).

### Helper report

What a helper sent for a task of a [work plan](#work-plan) hands in before
it ends: a summary, what it changed and ran, what it found and decided,
what is left, the risks and its confidence. HyperForm adds what it observed
itself (the files changed, the checks run) and keeps the helper's own
lists as claims. See
[The result contract](features-work-parallel.md#the-result-contract).

### Interrupted task

A task of a [work plan](#work-plan) whose helper stopped before it
finished: you paused the plan or held the task, its chat closed, or the
app restarted. It waits again with a [resume pointer](#resume-pointer),
and the interrupted start doesn't count against its attempts. See
[Resuming a plan](features-work-resume.md#resuming-a-plan).

### Native runtime

Running the [decision model](#decision-model) inside the app instead of as
a separate server, from files downloaded once in Settings and checked
against pinned checksums (`decisions.runtime = "native"`). See
[Reaching the decision model](features-experimental-and-decisions.md#reaching-the-decision-model).

### Needs you

What a [work plan](#work-plan) waits on you for: a merge conflict, a
change to apply, a helper that ended without a report or was discarded, a
proposed task or item, a task whose checks need to run again, and
high-severity [audit findings](#audit-finding). The Plan tab, the plan
list, the home card and the Inbox count the same. See
[The plan list and what needs you](features-work-resume.md#the-plan-list-and-what-needs-you).

### Parallel set

The tasks of a [work plan](#work-plan) that could start beside the
running ones now, the ones that can't (each with its reason: no files,
a [serial path](#serial-path), a dependency path, an overlap, no free
slot), the running tasks and the most allowed at once
(`work.max_parallel`). See
[Which tasks run together](features-work-parallel.md#which-tasks-run-together).

### Plan health

A [work plan](#work-plan)'s summary in counts and levels, never
percentages: requirements covered, tasks done, blocked and failed,
evidence (Good, Fair or Poor), stale evidence and open
[audit findings](#audit-finding). The Plan tab's coverage line shows it.
See [Plan health](features-work-parallel.md#plan-health).

### Proposed task

A task in a [work plan](#work-plan) that waits for you: added by a helper,
or by a turn that read web content while the plan was approved. Nobody
starts it until you **Accept** it in the Plan tab; **Dismiss** drops it.
See [Web content waits for you](features-work-plans.md#web-content-waits-for-you).

### Reconciliation

Comparing a [work plan](#work-plan) with the workspace, without a model:
deleted scope files, done tasks whose files changed, failed or replaced
dependencies, overlapping or duplicate tasks. A new problem sends the plan
back for replanning and [holds](#held-task) only the tasks it affects. See
[Reconciliation and /replan](features-work-plans.md#reconciliation-and-replan).

### Resume pointer

What an [interrupted task](#interrupted-task) keeps of its last run: the
chat and helper it ran in, the helper's worktree when there is one, and a
note on why it stopped. The main agent may continue that helper, or start
a fresh one, which drops the pointer. See
[Resuming a plan](features-work-resume.md#resuming-a-plan).

### Seam

One of the fixed points of a turn where a [decision use](#decision-use)
may give advice, each with limits; at the tool gate it can only turn an
allow into an ask. See [Seams](features-experimental-and-decisions.md#seams-where-a-decision-may-act).

### Scope drift

A write by a helper sent for a task to a file outside that task's
declared scope. It asks first, even when your rules allow it, and is
noted on the task. See
[Scope drift](features-work-parallel.md#scope-drift).

### Serial path

A file many tasks change as a side effect, such as a lockfile, a package
manifest or generated output (`work.serial_paths`). A task that names one
never runs beside others, only while nothing else runs. See
[Which tasks run together](features-work-parallel.md#which-tasks-run-together).

### Shadow mode

A [feature flag](#feature-flag) setting where the feature runs and records
what it would have done, but never changes a turn. See
[Seams](features-experimental-and-decisions.md#seams-where-a-decision-may-act).

### Sidecar

The [decision model](#decision-model) server HyperForm starts itself from
`decisions.sidecar.command`, once for the whole app, and stops on quit. See
[Reaching the decision model](features-experimental-and-decisions.md#reaching-the-decision-model).

### Soft stop

How pausing a [work plan](#work-plan), holding a task, closing a chat or
moving a plan stops a [task-bound helper](#task-bound-helper): it ends at
its next step and keeps its work, and its task becomes an
[interrupted task](#interrupted-task). **Stop** and **Abandon** cancel at
once instead. See [Pause, hold and stop](features-work-resume.md#pause-hold-and-stop).

### Task packet

The briefing on one task of a [work plan](#work-plan) that the agent gets
when it starts, when its task changes and after
[compaction](glossary.md#compaction): the goal, the rules that must hold,
the task and its criteria, then the most relevant notes, within
`work.packet_tokens`. See
[Task packets and compaction](features-work-plans.md#task-packets-and-compaction).

### Task-bound helper

A helper the main agent sent for one task of a [work plan](#work-plan)
(`Agent` with `task_id`). It owns that task until it ends, may change only
that task, and must hand in a [helper report](#helper-report). See
[Helpers on tasks](features-work-parallel.md#helpers-on-tasks).

### Task view

The working set a decision feature gives the model at a new task: earlier
exchanges the task doesn't need are set aside, saved to files and listed
in one index, while your transcript keeps everything. See
[Task-scoped history](features-decision-uses.md#task-scoped-history-and-its-record).

### Untrusted input

What a turn hands a [work plan](#work-plan) after it read web content
(`WebFetch`, `WebSearch`, or a helper that did), or a note citing a URL.
What it adds waits for you, marked **from web**, and it can't change what
the plan already says. See
[Web content waits for you](features-work-plans.md#web-content-waits-for-you).

### Work plan

With the experimental `work_plans` feature, a chat's work kept outside the
conversation: a goal, tasks with [acceptance criteria](#acceptance-criterion)
and a notebook of decisions, discoveries and evidence. A kernel checks
every change before it is saved. See [Work plans](features-work-plans.md).

See also: [Glossary](glossary.md) ·
[Glossary: the desktop app](glossary-desktop.md) ·
[Experimental features and the decision layer](features-experimental-and-decisions.md) ·
[Work plans](features-work-plans.md) ·
[Parallel tasks and the plan audit](features-work-parallel.md) ·
[Pausing, resuming and moving plans](features-work-resume.md)
