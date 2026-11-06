# Plan mode, questions and todos

Three features keep you in control of larger tasks: **plan mode** lets you
review a plan before anything changes, **structured questions** let the
agent ask you to choose between options, and the **todo list** shows its
progress step by step.

## Plan mode

In plan mode the agent researches and proposes; it changes nothing. It can
read and search files, run read-only commands such as `git log`, use the
language server, look things up on the web, and start `explore` or `plan`
subagents. Edits, and commands that change anything or that HyperForm
can't prove are read-only (such as `cargo test`), are refused with "plan
mode is read-only".

Plan mode is a switch on top of the
[permission mode](03-permissions-and-safety.md#permission-modes), not one
of the modes. Everything it doesn't refuse follows the permission mode:
with **Ask**, reading outside the project or fetching a web page still
asks; with **Bypass**, the agent researches without asking and still
changes nothing.

### Turn plan mode on or off

- Press **Shift+Tab** in the composer. A **Plan** chip appears beside the
  permission chip; press Shift+Tab again, or click the chip's **×**, to
  turn it off.
- Or choose **Plan first** (**Stop planning** while it is on) from the
  composer's **+** menu or the command palette.
- Or type `/mode plan`, which turns it on or off.
- To start every new chat in plan mode, turn on **Settings → Permissions →
  Start new chats in plan mode** (`permissions.planning = true`). The
  older `permissions.mode = "plan"` still works and means the same.

Then describe the task:

```text
Add rate limiting to the public API. Plan it first.
```

### Review the plan

When the plan is ready, a **Plan ready · Review** row appears in the chat
and the side panel opens on its **Plan** tab (**Review** opens it again),
marked **Ready for review**. It shows the plan in markdown: the goal, the
approach, the files to change, the ordered steps, how it will be verified,
and risks or open questions; when the agent keeps a todo list, its
**Checklist** and **N of M done** follow.

| Control | Effect |
|---|---|
| **Edit** / **Preview** | Switch to a text editor to change the plan yourself (Esc goes back to the preview). An **Edited** tag appears when your version differs. |
| **Build** | Turn plan mode off and start implementing. The permission mode stays as it is (its name is in the button's tooltip); change it with the permission chip first if you want to. |
| **Keep planning** | Opens a feedback box. Type what should change and click **Send feedback** (or press ⌘Enter / Ctrl+Enter). The agent stays in plan mode, revises, and proposes again. Esc closes the box. |

If you edited the plan, the agent is told to implement *your* version. While
the plan waits, the island in the title bar says **Plan ready for review**
in amber (its **Review** button goes to the plan), and the plan is listed
in the [Inbox](02-everyday-use.md#the-inbox). You can still send messages,
which are queued. With no plan waiting, the Plan tab shows the chat's
**Last plan** and its checklist, or **No plan yet**.

> **Tip:** Plans are only proposed for work that changes files. If you ask
> a question in plan mode, the agent simply answers it.

Only the main agent proposes plans; the `plan` subagent returns its plan as
a report to the main agent instead. While the chat is in plan mode, every
subagent it starts plans too and can't turn plan mode off.

## Plan mode with work plans

> **Note:** This section applies only while the experimental
> [work plans](18-work-plans.md) feature is on.

With work plans, the plan the agent proposes is a list of tasks the chat
keeps, each with acceptance criteria, and the **Plan** tab follows it
while it is built. With no plan yet, the tab says **No plan yet**;
**Start planning** turns plan mode on, and **Continue a plan** below it
lists the project's other plans
([continue a plan](19-pausing-and-continuing-plans.md#continue-a-plan-in-this-chat)).

### Review and build the plan

When the agent sends its plan, the tab says **Draft · ready for review**:

- **The header**: the goal (click it to see the requirements and
  invariants; the agent's suggestions say *proposed* until you build),
  counts such as "2 requirements · 6 tasks", **rev N**, which opens
  the notebook's **Changes**, and the coverage line
  ([below](#coverage-and-findings)).
- **Tasks** | **Document**: the task tree, or the plan as written. In a
  draft, **Edit** the document and **Save** it (⌘S / Ctrl+S).
- **Build** turns plan mode off and builds in this chat. Its arrow menu
  also offers **Build in a new chat**: a fresh chat gets the plan and
  starts from it, without this chat's research. With
  [parallel tasks](18-work-plans.md#parallel-tasks) on and at least two
  tasks that could run together, it also offers **Build with helpers in
  parallel** ("*N* tasks can run together").
- **Keep planning** sends feedback, as without work plans.

Building approves the plan: the proposed requirements and invariants
start to count, and the header says **Building** with "*N* of *M* tasks".
**Now** lists what runs: the main agent's task first, then one lane per
helper with its type (and "worktree" when it has one), its status, its
step ("3 of 5 steps") and "Criteria *X* of *Y* met"; a task whose
applied change is being checked says **checking**. Click a helper's tag
to open it in the **Agents** tab. With parallel tasks on, the header's
**⋯** menu has **Allow helpers in parallel**, or **Run one task at a
time** once allowed, and each task row says whether it **can run beside
others** or why not (**runs alone**, **overlaps** another task, and so
on; hover for the reason).

### What needs you

While the plan waits on you, an amber **Needs you** strip under the
header lists it, most urgent first:

- **Merge conflict in "*task*"**, with the files: **Retry serially**, or
  **Show worktree** to resolve it yourself
  ([when a change conflicts](18-work-plans.md#when-a-change-conflicts)).
- **"*task*" is ready to apply**: **Review** opens the **Agents** tab,
  where you **Apply** or **Discard** the helper's change.
- **"*task*" stopped**: it ended without a report, or its change was
  discarded. **Retry** it, or **Not needed** (after a confirmation).
- **"*task*" waits on checks that did not run or are stale**: **Run
  checks again** once you have fixed the cause.
- **Proposed requirement**, **Proposed invariant** or **Proposed task**:
  **Accept** or **Dismiss**.
- Open audit findings of **High** severity: **Add to plan** or
  **Dismiss**. After two, a line such as "3 more findings" opens the
  findings.
- "*N* tasks wait for you to accept the plan change", while the plan is
  replanning ([below](#change-pause-or-stop-the-plan)).

Click an entry to open its task. What counts:
[what needs you](19-pausing-and-continuing-plans.md#what-needs-you).

### Follow a task

Click a task for its detail: its status (**Ready**, **Waiting**, **In
progress**, **Verifying**, **Done**, **Blocked**, **Failed**, **Not
needed**, **Replaced**), who works on it, what it waits on, its
**Result**, its **Criteria** with the checks that ran (**stale** once
files changed since), its **Steps** (the agent's todo list for it), its
**Scope** files, and linked notebook entries.

The **Result** of a task a helper ran shows its summary, then what Z
Engine observed (the **Changed** files and the **Checks** that passed or
failed), then **The helper says (not checked)**: its own lists of files
and tests, blockers, follow-ups, risks and confidence, and what it
learned (marked **from web** when its run read a web page). Below come
how its change came back (waiting for you, applied, conflicting with
**Retry serially** and **Show worktree**, or discarded) and "Attempt *N*
of *M*".

- **Confirm** a criterion that you check yourself, once you have checked
  it. **Waive** a criterion the task shouldn't need.
- **Run checks again** appears while a task is **Verifying** and one of
  its checks didn't run or went stale.
- The **⋯** menu: **Rename**, **Hold** (it doesn't start, and its helper
  stops at its next step; **Release** undoes it), **Retry** a failed or
  blocked task (with a fresh set of attempts), **Run alone** (never beside
  other tasks; **May run beside others** undoes it), or **Mark not
  needed**, which lets the tasks that wait on it go ahead.
- A held task's row says **on hold**, and one that waits for a plan
  change says **waits for the plan change**. A task whose helper was
  interrupted says why, for example "Interrupted; its helper's work is
  kept in a worktree · the plan was paused", with **Show worktree** and
  **Its helper's transcript**
  ([interrupted tasks](19-pausing-and-continuing-plans.md#after-a-restart-or-when-a-chat-opens)).
- A **Proposed** task (added by a helper, or by a turn that read a web
  page, marked **from web**) waits for you: **Accept** it, on its row or
  in its detail, or **Dismiss** it.
- **Why?** lists the changes that made the task what it is; **What the
  agent was given for this task** opens its task packet in the
  **Context** tab.

The **Notebook** below the tasks keeps **Decisions**, **Discoveries**,
**Experiments** and **Changes** of plan. Entries marked **Unconfirmed**
(found while compacting, or recorded by a turn that read a web page) have
**Confirm** and **Dismiss**; entries from the web keep a **from web** mark. In the transcript, rows such as "Plan approved · rev 3",
"Task failed" or "Plan resumed" link to the change (**Why**) or the task
(**Open**).

### Coverage and findings

The coverage line in the header, such as "4 of 5 requirements covered ·
evidence Fair · 2 findings", is amber while findings wait for you. Click
it for **Findings**: open ones by severity (**High**, **Medium**,
**Low**), each with its kind (for example **Requirement not covered** or
**Risky path without a test**), what it is about, a **Suggested:** fix,
and **Add to plan** or **Dismiss**. Findings from the audit agent carry
an **audit agent** tag. Below come **Added to the plan**, then a folded
**Dismissed and resolved**. **Run audit** asks the audit agent to review
the plan now. Esc or the back arrow returns to the tasks. What the audit
checks: [The plan audit](18-work-plans.md#the-plan-audit).

### Change, pause or stop the plan

While it builds, the plan goes to **Replanning · review the changes**
when you turn plan mode on, choose **Request replanning** from the
header's **⋯** menu, run `/replan`, or problems were found in the files
under the plan ([what is checked](18-work-plans.md#what-changes-for-the-agent)).
When you asked for it (plan mode, **Request replanning**, or `/replan`
with your own request), no task starts until you decide. When problems
were found, only the tasks they name and the tasks that wait on them
hold (**waits for the plan change**); the other tasks keep running. The
bottom says what changed **Since you approved** (added, changed, replaced
or removed tasks) and why; click **Accept changes** to keep building, or
**Keep planning** with feedback.

The header's **⋯** menu offers what the plan's state allows:

| Item | When | What it does |
|---|---|---|
| **Pause** | building or replanning | Nothing new starts; running helpers stop at their next step and keep their work. |
| **Request replanning** | building | The agent revises the plan. |
| **Stop** | building or replanning, or paused while helpers still hold tasks | After a confirmation, cancels every helper now and pauses the plan. |
| **Abandon the plan** | any state but done or abandoned | After a confirmation, nothing more runs; the plan stays, read-only. |
| **Switch to another plan…** | the project has other plans | Lists them ([continue a plan](19-pausing-and-continuing-plans.md#continue-a-plan-in-this-chat)). |
| **Detach from this chat** | always | Removes the plan from this chat (its helpers stop) and keeps it on disk. |

A paused, done or abandoned plan shows a banner over its tasks:
**Paused** with **Resume** (and **Stop** while helpers still wind down),
**Done** with what was finished and how it was verified and **Reopen**,
or **Abandoned** (read-only) with **Reopen**.
[Pausing and continuing plans](19-pausing-and-continuing-plans.md)
explains what each one does.

## Questions from the agent

When a decision materially changes the result (two sound designs, a
destructive step, a product choice), the agent can ask you one to four
multiple-choice questions at once. A **Question** card takes the
composer's place (your draft is kept for later; an approval waiting at the
same time comes first):

- With several questions, tabs at the top switch between them; a check mark
  shows which are answered.
- For a single-choice question, clicking an option selects it and moves to
  the next question. For a multiple-choice question ("Pick any that
  apply"), click every option that applies.
- **Other…** lets you type your own answer. Press Enter in that box to
  submit once everything is answered.
- **Submit** sends your answers (it is enabled when every question has an
  answer).
- **Dismiss** declines to answer. The agent is told not to ask again and to
  continue on its best judgment or explain what is blocked.

While a question waits, the island says **Question for you**, with an
**Answer** button that goes to it. Only the main agent can ask questions;
subagents can't.

## Todos

For tasks with several steps, the agent keeps a todo list with the
`TodoWrite` tool. Each item is pending, in progress, or completed; the agent
keeps exactly one item in progress at a time.

- **The island** in the title bar shows the current step, such as "Running
  the parser tests", and a ring around the pet fills as items are
  done.
- **Click the island** to open it: its **Plan** row says how many items are
  done and which one is current (for example **2 of 5 · Run the tests**);
  click it for the full checklist in the side panel's **Plan** tab.
- **`/todos`** prints the list in the transcript as a checklist.
- A subagent's own todo list is shown in its transcript in the side
  panel's **Agents** tab.

The todo list survives [compaction](07-memory-and-context.md#automatic-compaction),
so it also serves as the agent's working memory in long sessions. If the
agent has worked for a while on a multi-step task without a list, it gets a
reminder to create one.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Agents](04-agents.md) · [Everyday use](02-everyday-use.md)
