# Everyday use

This page covers what you'll do most days: starting from the project home,
asking questions, fixing bugs, adding features, reviewing and committing,
attaching files, steering the agent, following its progress (with your
pet), the side panel, and undoing work.

## The project home

With no chat open, the main area shows the active project's home: the
project name and git branch, **What should we work on?**, and the composer
in the middle. Below the composer:

- **Starter chips** fill the composer with a first prompt that fits the
  project, such as **Review my 3 uncommitted changes**, **Write an AGENTS.md
  for this project** or **Explain how this project works**. **More ideas**
  shows the rest.
- **Continue** lists your three latest chats in the project with where each
  stands, its age and its cost; with [work plans](19-pausing-and-continuing-plans.md#from-the-project-home-and-the-inbox), also the plan in progress.
- **Uncommitted changes** lists the first changed files; click one, or
  **Review all N**, to open them in the [Changes tab](#review-changes-in-the-changes-tab).
- **Set up this project** lists what is still missing (**Connect a model**,
  **Trust this project**, **Add an AGENTS.md**), each with a button, and
  disappears once everything is done.

When you send the first message, the composer glides to the bottom and the
chat begins. **Home** in the sidebar and **New chat** (⌘N / Ctrl+N) bring
you back here. Without any project, the home offers **Add a project**.

## Ask questions about code

Just ask. The agent searches and reads the project and answers with
references like `src/billing/invoice.rs:42`.

```text
Where do we validate email addresses, and which endpoints use it?
```

For broad questions the agent may start an `explore` subagent that searches
in parallel and reports back (see [Agents](04-agents.md)). Reading files in
the project never needs approval.

## Fix a bug

Describe the symptom and, if you have it, paste the error:

```text
`npm test` fails in auth.spec.ts with "token expired" even for fresh tokens.
Find the cause and fix it.
```

The agent typically reproduces the problem, edits the code, and runs the
tests again. The turn's badge tells you whether a passing test run is newer
than its last edit ([Verification](10-verification.md)).

## Add a feature

For anything larger than a few lines, consider [plan mode](05-plan-mode-questions-and-todos.md)
first (Shift+Tab turns it on and shows a **Plan** chip): the agent
researches, proposes a plan, and changes nothing until you build it.

## Review and commit

- `/review` reviews your uncommitted changes, or a target you name
  (`/review src/api`, `/review main...HEAD`). It uses the `review` agent and
  reports findings by severity; it does not change code.
- `/security-review` does the same with a security focus.
- `/commit` creates one commit in your repository's message style. It may
  run only `git status`, `git diff`, `git log`, `git add` and `git commit`
  for that turn, and it never pushes. Add a hint: `/commit only the parser
  changes`.

Read the changes yourself in the [Changes tab](#review-changes-in-the-changes-tab);
every built-in command is in [Commands and skills](06-commands-and-skills.md).

## The composer

The composer's bar holds the **+** menu (**Attach images**, **Mention a
file or agent**, **Run a command**, **Save a note to memory**, **Run a shell
command yourself**, and **Show the terminal** when shell output is hidden;
each item types its character for you; then **Plan first** or **Stop
planning**), the **Plan** chip while
[plan mode](05-plan-mode-questions-and-todos.md) is on, the permission chip
([permission modes](03-permissions-and-safety.md#permission-modes)), the
model chip with the reasoning effort
([choosing a model](11-models-providers-and-cost.md#choosing-a-model-for-a-chat)),
and Send. While the agent works and the draft is empty, Send turns into a
square **Stop** button; once you type, it queues your message instead and a
small **Stop** button sits beside it. An approval or a question that waits
for you takes the text box's place, keeping your draft, until you
[answer it](03-permissions-and-safety.md#answering-approval-cards).

| You type | What happens |
|---|---|
| Text + **Enter** | Sends the message; while the agent works, queues it ([below](#steer-interrupt-cancel)). **Shift+Enter** inserts a new line. |
| `@` | Lists matching project files and agents. |
| `/` | Lists slash commands, grouped as Prompts, Session and App. |
| `!command` | Runs a shell command yourself (shell mode). |
| `# note` | Saves a note to an instruction file ([memory](07-memory-and-context.md#save-a-note-with-)). |
| **↑ / ↓** | Walk through your earlier messages (from the first or last line). |
| **Shift+Tab** | Turn [plan mode](05-plan-mode-questions-and-todos.md) on or off. The permission mode stays as it is. |
| **Esc** | Cancel the running turn, or clear the draft when idle. |

In the pop-up lists, ↑/↓ move, Enter or Tab picks, and Esc closes the list.

### Attach files with @

Type `@` and part of a file name, then pick a file from the list. The file
is attached to your message as a chip above the input, and `@path` is
inserted into your text. When you send:

- text files are included (up to 256 KiB each) and count as already read,
  so the agent can edit them straight away;
- PDFs are sent as documents (up to 32 MiB);
- images are sent as images.

Remove an attachment with the × on its chip. An `@path` typed by hand,
without picking it from the list, attaches nothing: the agent sees the text.

### Attach images

Paste an image, drag it onto the composer (it shows **Drop images to attach
them**), or choose **Attach images** in the **+** menu: up to six per
message, scaled down to at most 1568 pixels on the long side before sending.

> **Note:** Attachments are only sent with a new prompt. If you send while
> the agent is working, the attachments stay in the composer for your next
> prompt.

### Run a shell command yourself

Start a message with `!` to run a command in the project without involving
the model, for example `!git status`; the composer shows a **Shell** tag
and a **Run** button. The output appears in the **Shell** drawer above the
composer, with buttons to copy it, make it taller, clear it and hide it
(Esc); **Show the terminal** in the **+** menu brings it back.

These commands run directly on your machine, without approval prompts and
outside the sandbox, with a 10-minute limit. A command that isn't read-only
counts as a change for the next turn's badge.

## Steer, interrupt, cancel

You don't have to wait for the agent to finish. While it works, the
placeholder reads "Add to what the agent is doing…"; once you type, the
composer shows **Enter queues · ⌘Enter interrupts** (Ctrl+Enter elsewhere).

- **Steer:** type and press **Enter**. The message is queued (shown as
  pills labelled *queued* above the input) and delivered at the next step,
  between tool calls. Click a pill to edit it, or × to remove it. In the
  transcript it appears as a **Steered** note.
- **Interrupt:** press **⌘Enter** / **Ctrl+Enter**, or click that part of
  the hint: the current step stops and your message is sent right away as a
  new turn (the agent is told it was interrupted).
- **Cancel:** press **Esc** or the square **Stop** button. The turn ends;
  unfinished tool calls are cancelled. Background jobs keep running.
  Messages still queued are kept and sent with your next prompt.

Slash commands that start a turn (like `/review`) can't be queued: wait
until the agent is idle.

## The title bar

The middle of the title bar is the **island**: your [pet](#your-pet) and
one line with at most one number. While the agent works, the line says what
it is doing ("Reading auth.rs"), the number is the elapsed time, and a ring
around the pet fills as the todo list gets done. When a turn ends, it
briefly shows the result (**Verified**, **Checks failed**) and the turn's
duration. It says **Provider busy** with a countdown while a request is
retried, turns amber when this chat needs you (with **Approve**,
**Answer** or **Review** to go to it), and shows the chat's title when idle.

Click the island to open its card: the whole message, the plan (done of
total, current item), the latest steps (**Now**), **Agents**, **Context**
and **Waiting** rows that open what they sum up, the cost of this turn and
the chat, and recent warnings (a dot on the island means some are new).
Long notices, errors and notices with buttons open it by themselves, and it
closes when the notice ends unless the pointer is over it; a click
elsewhere or Esc closes it. **Open in Inbox** shows a notice in the
[Inbox](#the-inbox).

Left of the island, an amber number counts other chats that need you:
click it to open that chat, or the Inbox when there are several. Right of
it, a ring shows how full the context is (a percentage from 65%, amber, red
from 85%); click it for the [context card](07-memory-and-context.md#the-context-window).
At the right, the changes button (**Review changes**, ⌘D / Ctrl+D) counts
the files this chat changed and opens the [Changes tab](#review-changes-in-the-changes-tab);
the next button shows or hides the [side panel](#the-side-panel). While
the sidebar is hidden (⌘B / Ctrl+B), **New chat**, search and **Settings**
move to the title bar. Double-click an empty part of the title bar to
maximize or restore the window.

## Your pet

The pet is a small 3D creature, named **Zen** until you rename it, that
lives in the island (a flat version stands in where 3D can't be drawn). Its
face echoes the status line: busy while the agent works, amber and looking
at you when something needs you, cheering when a turn is verified, drooping
when one fails, sleepy after quiet minutes, and greeting you when the app
opens. It never tells you anything the line doesn't.

At the default **Lively** level with **Let it roam** on, the pet leaves the
island while nothing needs you, and the island keeps a small round portrait
of it that turns to watch it. It sits above **What should we work on?** on
the project home, stands on the composer's top edge in a chat (and now and
then walks to the sidebar's bottom edge or sits in the side panel's tab
bar), sits in an empty Inbox, watches the composer while you type, and naps
after 3 minutes without activity. It rides back into the island while the
agent works, hops in when something needs you, and tucks itself away while
a menu or the palette is open. Drag it to the nearest spot, click to boop
it, and double-click or right-click it for its card. Under Reduce Motion it
appears in its new spot instead of walking there and does no idle strolls
or tricks, and the portrait holds still.

The **pet card** shows its name, level, stage, XP to the next level, day
streak and counts of **Turns**, **Verified** and **Applied**. **Wears**
puts on an unlocked accessory; **Customize…**, **Stop roaming** (or **Let
it roam**) and **Call back** are there too.

It grows from finished work in any chat: 10 XP per completed turn, 15 more
when its checks pass, 20 per applied helper worktree, and 5 more for the
first of these each day; nothing is ever taken away. Levels need 50, 150,
300, 500… XP in total (up to 99). Its shape changes at levels 3, 6 and 10;
a sprout, scarf, headphones and star unlock at 2, 4, 6 and 9, and tricks
(stretch, hop, spin, sparkle) at 1, 3, 5 and 7. At **Lively** a new level
brings confetti once nothing needs you. Growth is saved in `pet.json` in
the [data folder](13-sessions-and-data.md#where-things-live).

To change the pet, open **Settings → Pet**: **Name** (up to 24 characters),
**Look** (Pearl, Mint, Sky, Lilac, Peach or Graphite), **Liveliness** and
**Let it roam** ([settings](12-settings-reference.md#ui-and-compatibility)).
**Calm** reacts only to the agent and stays in the title bar; **Off** shows
a small dot instead. With roaming off, the pet stays in the island except
for its spot on the project home. The palette has **Rename**, **Change
look**, **Show card**, **Stop roaming** and **Call back**, named after your
pet (**Rename Zen**).

## The Inbox

**Inbox** in the sidebar collects what is going on across all your chats:

- **Needs you**: approvals, questions, plans, trust requests and work
  plans waiting on you. Approvals can be answered here (**Allow once**,
  **Deny**); **Open chat**, **Answer**, **Review plan**, **Decide** and **Open plan** open the chat.
- **Finished while you were away**: chats whose turn ended while you were
  elsewhere, with how each ended; **Open** opens one.
- **Notices**: every notice in full, including each chat's warnings, errors
  and blocked hooks, newest first. Long ones fold (**Show all**), each has a
  copy button, and **All** / **Problems** filters them.

The Inbox badge counts what needs you, finished chats you haven't opened,
and problems since you last looked; opening the Inbox marks them as read.
With nothing waiting it says **You're all caught up**. It is kept only
while the app runs ([Sessions and data](13-sessions-and-data.md#where-things-live)).

## Reading tool runs and cards

When a turn with two or more tool calls is done, its work folds into one
line above the answer, such as "Worked for 1m 12s · read 6 files · ran 2
commands" (plus **1 failed** or **1 denied**); failed calls, helpers,
questions and plans stay visible under it, and a click shows every step.
Inside, tool calls in a row fold the same way ("Read 2 files · searched 1×
· edited 1 file"), with the step in progress and the number of steps; a
running command shows its last three lines, a failure the last three lines
of its output. Click such a line for each call's own card; agent, question
and plan calls always keep their own.

At the end of each turn, small buttons copy the answer, open its changes
and [rewind](#rewind). Once a chat has two prompts or more, a rail of dots
at its right edge marks them (up to 40): hover one to read the prompt,
click it to jump there.

Every call is a one-line card: an icon, the tool name, its subject (a file
path, command or URL) and the time it took. The dot and tag show the
status: running, done, failed, **denied** (by you, a rule or a hook) or
**cancelled**. Click a card to expand it:

- **Edit/Write** cards show the diff with added and removed line counts,
  **Bash** cards the command, its live output (the last three lines under
  the card while it runs) and the exit code; both open by themselves only
  on a failure.
- **Agent** cards show a tiny sprite of the pet (it bobs while the helper
  works), the subagent's type, task, status and time, with
  **Open** for its transcript. Click the line for its model, tokens, cost,
  tool calls, worktree state and result.
- Other cards show the tool's output.

### The turn receipt

Under each turn, a one-line *receipt* sums up how it ended. How much it
shows is set in **Settings → Appearance → Task report detail**
(`ui.task_report_view`):

| Detail | The receipt shows |
|---|---|
| **Quiet** (default) | The verification badge, only when it is **Verified**, **Unverified** or **Failed** ([Verification](10-verification.md)). |
| **Compact** | Also the files the turn changed (four chips, then **+N**; click one to see it in the Changes tab), the duration and the cost. |
| **Detailed** | Also the tokens, the **Not applicable** badge of turns that changed nothing, and the checks unfolded. |

Notes such as **Cancelled**, **Failed**, **Stopped** or **Interrupted**
always show. Hover a receipt for the turn's time, tokens and cost.
[Experimental decision features](16-decision-features.md) may add
**Claimed, not checked** here and cards (plan first, save a rule, run a review).

## The side panel

The side panel opens beside the chat with four tabs: **Changes** (below),
**Plan** ([the plan](05-plan-mode-questions-and-todos.md#review-the-plan)),
**Agents** ([helpers and jobs](04-agents.md#watching-agents)) and
**Context** ([what the model was sent](07-memory-and-context.md#inspect-the-prompt)).
Drag its left edge to resize it (the width is remembered); **Use the whole
stage** covers the chat, **Dock beside the chat** brings it back. A dot on
a tab means news there; a plan waiting for review opens Plan by itself.
← / → switch focused tabs; **Esc** steps back (out of a helper's
transcript, out of the whole stage, then closed).

## Review changes in the Changes tab

The changes button in the title bar (⌘D / Ctrl+D; again to close) opens
the side panel on **Changes**; its header counts the files and the
lines added and removed. Choose **This chat** (every file that changed in
the project since this chat's first code checkpoint) or **Uncommitted**
(all uncommitted changes compared with your last commit), and **Unified**
or **Split** (one column, or old and new side by side; remembered on this
computer). In a narrow panel the layout switch hides, and at its narrowest
the file list sits above the diff.

With several files, a list on the left groups them by folder, marks each
**A**, **M**, **D** or **R** (added, modified, deleted, renamed) with its
line counts, and offers a filter past six files. Each file's bar has
**Open** (in its default app), a copy-diff button and a **…** menu:
**Reveal in Finder** (**Show in Explorer** on Windows, **Show in file
manager** on Linux), **Copy path**, **Copy relative path** and **Copy
diff**. Long unchanged stretches fold into **Show N unchanged lines**; very
long diffs stop after 400 lines with **Show the remaining N lines**.

While you're not typing, `]` or `j` and `[` or `k` go to the next and
previous file. A finished turn refreshes the tab, and a file chip in a
receipt or on the project home opens it at that file.

## Rewind

Click **Rewind** at the end of a turn (Rewind to before this prompt):

| Option | Effect |
|---|---|
| **Code and conversation** | Restore the files to how they were before this message, and drop this message and everything after it. |
| **Conversation only** | Drop this message and everything after; keep the files as they are now. |
| **Code only** | Restore the files; keep the conversation. |

When you rewind the conversation, your message goes back into the composer
to edit and resend. A running turn is cancelled first; a notification lists
the files restored, deleted or left alone.

> **Warning:** Rewind restores files, not the outside world. Installed
> packages, database changes, pushes and other side effects of commands are
> not undone. Files ignored by git and files larger than 8 MiB are not part
> of checkpoints. Details: [Sessions and data](13-sessions-and-data.md#checkpoints-and-rewind).

## Resume, manage and export chats

- Every chat is saved automatically; reopen it from the sidebar, also
  after restarting the app.
- The sidebar lists chats under their project, with their age (`5m`, `3h`,
  `2d`). A dot before a chat means it is working, needs you (amber),
  finished while you were away, or its last response didn't complete; hover
  it for the exact state (the open chat shows none: the title bar reports
  it). A project lists eight chats, then **Show N more**; chats whose folder
  isn't a project are under **Other chats**.
- Click a project to fold or unfold it. Hover it for **+** (a new chat
  there), or right-click it for **New chat here**, **New chat in a
  worktree…**, **Reveal in Finder**, **Copy path** and **Remove project…**.
- ⌘K / Ctrl+K or **Search** in the sidebar opens the
  [command palette](06-commands-and-skills.md#the-command-palette) (actions,
  recent chats, projects and settings); `/resume` opens it with chats only.
- `/clear` or **New chat** starts a fresh chat; the old one stays.
- `/export` copies the chat to your clipboard as Markdown (tool calls
  summarized, internal reminders hidden), `/export json` every recorded
  event as JSON (palette: **Copy the chat as Markdown** / **as JSON**).

> **Warning:** Deleting a chat (the trash icon next to it) asks you to
> confirm, then removes it for good. Removing a project from the sidebar
> also deletes all of its chats once you confirm. Your project files are
> not touched.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Commands and skills](06-commands-and-skills.md) · [Sessions and data](13-sessions-and-data.md)
