# Work plans

**Work plans** (`work_plans`) is an [experimental feature](15-experimental-features.md)
that keeps the work of a chat as a plan of tasks that lasts beyond the
conversation. Each task has acceptance criteria, and a task counts as done
only when its checks pass on the code as it is now. The main agent can
hand a task to a helper, and with **Parallel tasks** (`work_parallel`)
independent tasks run at the same time, each in its own worktree. A plan
audit points out what the plan misses. You can pause a plan or hold one
task, and pick a plan back up after a restart or in another chat.
Neither feature uses the decision model.

This page explains what the features do and how to use them. What the
**Plan** tab shows, click by click, is in
[Plan mode with work plans](05-plan-mode-questions-and-todos.md#plan-mode-with-work-plans).

## Turn it on

In **Settings → Experimental**, set **Work plans** to **On** (it has no
Shadow), or add `work_plans = "on"` to `[experimental]`. While it is on:

- the agent has five more tools: `TaskCreate`, `TaskUpdate`, `TaskGet`,
  `TaskList` and `PlanRecord`
  ([tools](../../README.md#tools)), and its `Agent` tool can send a helper
  for a task;
- its `ExitPlanMode` plan can carry a goal, requirements and invariants;
- the built-in [`audit` agent](04-agents.md#built-in-agents) is offered;
- `/replan` appears in the command menu
  ([commands](06-commands-and-skills.md#session-commands));
- the side panel's **Plan** tab shows the chat's plan.

To let tasks run side by side, also set **Parallel tasks** to **On**
(`work_parallel = "on"`). It needs work plans: on its own it stays off,
with the warning "experimental.work_parallel needs work_plans; it stays off
until work_plans is on".

```toml
[experimental]
work_plans = "on"
work_parallel = "on"
```

## Plan and build a job

1. Turn on plan mode and describe the job
   ([Plan mode with work plans](05-plan-mode-questions-and-todos.md#plan-mode-with-work-plans)).
   The agent explores, creates the tasks with their criteria and sends
   its plan; the **Plan** tab shows it as a draft.
2. Review it: edit the document, open tasks, read the
   [audit's findings](#the-plan-audit), or click **Keep planning** with
   feedback.
3. Click **Build** to build in this chat, or choose **Build in a new chat**
   from its menu to start fresh with only the plan. With parallel tasks
   on, the same menu offers **Build with helpers in parallel**
   ([below](#parallel-tasks)).
4. While it builds, the agent works through the ready tasks, itself or
   through [helpers](#helpers-on-tasks), runs their checks with `Verify`
   and marks them completed. Follow along in the **Plan** tab; failed
   tasks and changes of plan also show in the transcript.

Without plan mode the agent may still create tasks for a larger job; the
plan then starts building at once, with no draft to review.

## What changes for the agent

- **Done needs proof.** A criterion such as "the client tests pass" is
  met only by a passing `Verify` check of that kind on the current code;
  a command run with `Bash` doesn't count. The agent may confirm a
  criterion it checks by reading code; a criterion you check yourself
  only you can confirm (or waive). Until then HyperForm refuses to mark
  the task done and tells the agent what is missing.
- **A task packet.** When it starts work, after its task changes and after
  a compaction, the agent gets a briefing on its task: the goal, the rules
  that must hold, the task's criteria and latest results, its
  dependencies, and the decisions and findings that matter to it, within
  `work.packet_tokens`. See it in the **Context** tab under **Task
  packet** ([Inspect the prompt](07-memory-and-context.md#inspect-the-prompt)).
- **Notes that last.** Decisions, discoveries and experiments the agent
  records with `PlanRecord` stay in the plan. Facts found by a compaction
  summary are kept as *unconfirmed* until you confirm them.
- **Web content waits for you.** Once the agent reads a web page in a
  turn (`WebFetch`, `WebSearch`, or a helper that did), what it adds to
  the plan for the rest of that turn is marked **from web** and waits for
  you: a new task in a building plan is **Proposed** until you click
  **Accept** (or **Dismiss**) on its row or in its detail, and its notes
  are unconfirmed until you **Confirm** them. In a draft, **Build**
  approves its tasks with the rest. That turn can't rename, rescope,
  rewire or cancel tasks already in the plan; the agent tells you what it
  would change and you make the change in the **Plan** tab. Your next
  message starts clean.
- **The plan is checked against your files.** When files changed since
  the last turn, HyperForm compares the plan with the workspace. If a task
  relies on a file that was deleted, a done task's files changed, two
  open tasks overlap, or a task waits on one that failed or was replaced,
  the plan goes to **Replanning** and the agent is asked to revise it.
  Only the tasks the problem names, and the tasks that depend on them,
  wait ("waits for the plan change"); every other ready task keeps
  starting and running. They go on once you accept the changes in the
  **Plan** tab, or once the problem no longer stands. A problem already
  reported isn't reported again while it stands. `/replan` runs this
  check on demand. A done task's files count
  as changed only when they changed after it was done, so its own edits
  never do; an edit from outside made in the same turn right after the
  task finished isn't caught.
- **Blocked by a failed task.** A task that waits on one that failed, or
  on one that was replaced, is **Blocked** until that task is done or
  marked not needed, or the dependency is removed.
- **A retry limit.** Agents may start the same task at most
  `work.max_attempts` times (3). After that only you can **Retry** it,
  which gives it a fresh set of attempts.

The `[work]` settings are file-only
([Work settings](12-settings-reference.md#work)).

## Helpers on tasks

The main agent can send a [helper](04-agents.md) for one ready task of
the plan. The helper is told which task it owns, gets the task's packet,
and works on that task only:

- It may update its own task: its status, criteria, steps and notes. A
  subtask it creates is **Proposed** and waits for you; any other change
  to the plan is the main agent's.
- A write outside the task's declared scope asks you first, even when
  your rules would allow it
  ([scope drift](03-permissions-and-safety.md#approvals-from-experimental-features)),
  and each such file is noted on the task.
- Before it ends, it reports its result: a summary, what it changed and
  ran, what it found and decided, what is left, the risks, and how sure
  it is.

When the helper ends, HyperForm records the result in the task's
**Result**: what it observed itself (the files the change touched and the
checks the helper ran) first, then what the helper says, marked *not
checked*. The last report it sent counts, however its run ends. A
helper that ends without a report leaves its task **Blocked** ("ended
without a report"); **Retry** it, or mark it **Not needed**. After each result the main agent is told what happened and
which tasks are ready next.

The Agents tab and the transcript's **Agent** card name the task a helper
works on; click the name to open the task in the **Plan** tab. Without
parallel tasks, only one helper works on a task at a time.

## Parallel tasks

With **Parallel tasks** on and your approval, independent tasks of a
building plan run at the same time, each in a helper with its own
[worktree](04-agents.md#worktree-isolation), and their changes come back
into your project one at a time.

### Approve parallel work

Only you can allow it, per plan:

- In a draft, the **Build** menu offers **Build with helpers in
  parallel**, with "*N* tasks can run together", once at least two tasks
  could. It builds the plan and allows parallel work in one step.
- While a plan builds, its **⋯** menu offers **Allow helpers in
  parallel**, and once allowed **Run one task at a time**, which stops new
  tasks from starting beside others (running helpers finish).

Until you allow it, the agent can't send a helper for a task while
another task is in progress; it waits for that task to finish.

### Which tasks run together

A task runs beside others only when nothing links it to them. Task rows
carry a quiet badge, and its tooltip says why:

| Badge | Why |
|---|---|
| can run beside others | It is ready and could run in a helper now. |
| runs alone | It names no files (so an overlap can't be ruled out), it touches a [serial path](12-settings-reference.md#work) such as `Cargo.lock` or `package.json`, you chose **Run alone** in its **⋯** menu, or it was retried one at a time after a merge conflict. |
| not beside *task* | That task runs alone, so nothing starts beside it. |
| ordered with *task* | A dependency links the two. |
| overlaps *task* | They share files or symbols. |
| waits for a helper | `work.max_parallel` tasks already run. |

At most `work.max_parallel` tasks (3) run at once, and never more than
`agents.max_concurrent`.

### Bring the changes back

When a helper in a worktree reports, its change waits for you, as with
any worktree agent. The task's **Result** says "The helper's change waits
for you to apply it", and **Needs you** says the task "is ready to apply".

1. Click **Review** to open the **Agents** tab.
2. **Apply** brings the change into your project folder, and the task
   shows **Verifying**. HyperForm runs, once in your project, each kind of
   check the task's criteria name. When they pass and its other criteria
   hold, the task is **Done**; when one fails, the task **Failed** ("the
   integration check … failed in the project tree"). A check run for
   the main agent in your project (its `Verify`, or the
   [automatic checks](10-verification.md#verification-modes) at the end of a turn) counts
   for a Verifying task too.
3. If a check couldn't run, or its result is stale, the task stays
   **Verifying**. Fix the cause, then click **Run checks again** in the
   task's detail or in **Needs you** ("… waits on checks that did not run
   or are stale"). **Waive** the criterion instead if it doesn't apply.
4. **Discard** drops the change and blocks the task ("its change was
   discarded"): **Retry** it or mark it **Not needed**.

A helper that worked in your project folder needs no apply step: it
marks its task completed itself, or the task's checks run in place when
it ends. With `merge = "auto"` in `[work]`, a change is applied as soon as
its helper ends, still one at a time; if that fails, a notice says why
and the change waits for you. A project can set
`"auto"` only once you
[trust it](03-permissions-and-safety.md#workspace-trust).

### When a change conflicts

If applying conflicts with your files, nothing in your project changes,
the worktree is kept, and the task is **Blocked**. **Needs you** says
"Merge conflict in *task*" with the conflicting files:

- **Retry serially** drops the conflicted change and runs the task
  again, alone, on your current code, with a fresh set of attempts.
- **Show worktree** opens the kept worktree, so you can resolve the
  files there and click **Apply** again in the **Agents** tab; the task's
  checks then run as usual.

## The plan audit

Each time the plan changes, HyperForm checks it and lists what it misses
as *findings*:

| Finding | Severity |
|---|---|
| A requirement with no task, or none with criteria | High |
| A criterion that needs a kind of check the project has none of | High |
| A task touching a risky path (migrations, SQL, schemas, protocol, storage, or your `work.sensitive_paths`) with no test criterion | High |
| An open task without criteria | Medium |
| A risk noted without a mitigation | Medium |
| Open tasks whose scopes overlap with no order between them | Medium |
| Open tasks with the same title | Low |

The read-only [`audit` agent](04-agents.md#built-in-agents) reviews the
plan too: when the agent ends planning with a plan that needs deep
planning (eight or more open tasks, files in five or more folders, or a
task touching one of your `work.sensitive_paths`, judged from the whole
plan then and again when you approve it), and whenever you click **Run
audit** in the findings. It reads the plan and answers with findings; it
changes nothing. That review is one request to your review model, without
tools, counted in the chat's cost. If it can't finish, a notice says "The
plan audit did not finish: …". The main agent can also send `audit` as a
helper, which then reads your code with `Read`, `Glob` and `Grep`.

The coverage line in the plan's header sums it up, for example "4 of 5
requirements covered · evidence Fair · 2 findings" (amber while findings
are open). Evidence shows once a task is done: **Good** when every
criterion of every done task is met (none waived), **Poor** when a done task has no
criteria or a failed one, and **Fair** otherwise. Click the line for the
findings, grouped by severity:

- **Add to plan** turns a finding into a task (from its suggestion) and
  tells the agent to address it.
- **Dismiss** sets it aside; HyperForm's own checks don't raise it again.
- A finding from HyperForm's own checks that no longer applies is resolved
  on its own; one from the audit agent stays until you add or dismiss it.

## Pause and continue a plan

You can pause a plan, hold one task, stop or abandon the plan, and pick
it back up after a restart, in another chat, from the project home or
from the Inbox. A plan whose last task is done becomes **Done** by
itself. See [Pausing and continuing plans](19-pausing-and-continuing-plans.md),
which also says what counts as
["needs you"](19-pausing-and-continuing-plans.md#what-needs-you).

## Limits in this version

- A chat has one plan at a time, and one chat drives a plan. To start an
  unrelated plan in a chat whose plan is still open, detach it first; once
  a plan is done or abandoned, the agent's next new task starts a new one.
- Only the main agent sends helpers for tasks; a helper can't start
  another task.
- Only `WebFetch` and `WebSearch` count as reading the web. A page
  fetched with `curl` through `Bash`, or content from an MCP tool, doesn't
  mark the turn.
- Turning the feature off hides the plan and its tools but keeps the
  plan on disk. Turn it on again and the plan is back; a chat opened while
  the feature was off finds it the next time that chat opens. While it is
  off, a helper the agent sends is an ordinary one: it isn't tied to any
  task.

> **Note:** Plans are stored on this computer, in `plans/` in the data
> folder ([where that is](13-sessions-and-data.md#where-things-live)), and
> are kept when you delete a chat. A task's worktree, an interrupted
> task's too, stays in `.hyperform/worktrees/` until its change is applied,
> discarded or retried serially. The packets, task lists and the plan the audit reads go to
> your model provider like the rest of the conversation.

See also: [Plan mode with work plans](05-plan-mode-questions-and-todos.md#plan-mode-with-work-plans) ·
[Pausing and continuing plans](19-pausing-and-continuing-plans.md) ·
[Agents](04-agents.md) · [Experimental features](15-experimental-features.md) ·
[Work settings](12-settings-reference.md#work) ·
[How work plans work](../how-it-works/features-work-plans.md) ·
[How parallel tasks work](../how-it-works/features-work-parallel.md) ·
[Troubleshooting](14-troubleshooting.md#a-work-plan-refuses-a-change)
