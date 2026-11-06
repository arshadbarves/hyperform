# Pausing and continuing plans

With [work plans](18-work-plans.md) (`work_plans`, experimental) on, a
plan outlives the chat that made it. You can pause a plan or hold one of
its tasks, stop or abandon it, and pick it back up after a restart, in
another chat, from the project home or from the Inbox. This page explains
how. The buttons themselves are in
[the Plan tab](05-plan-mode-questions-and-todos.md#change-pause-or-stop-the-plan).

## Pause, hold or stop the work

| To… | Click | What happens |
|---|---|---|
| Pause the plan | **⋯** → **Pause** | Nothing new starts. Running helpers stop at their next step (at once if one waits on your approval); their tasks wait, and their worktrees are kept. **Resume** in the **Paused** banner goes on. |
| Hold one task | the task's **⋯** → **Hold** | The same, for that task only. It shows **on hold** and doesn't start until you click **Release**. |
| Stop now | **⋯** → **Stop**, or **Stop** in the banner | After you confirm, every helper is cancelled at once, without finishing its step, and the plan is **Paused**. Its tasks wait and start afresh when you resume. |
| Give the plan up | **⋯** → **Abandon the plan** | After you confirm, helpers are cancelled at once and the plan turns read-only. |

- Closing a chat, or detaching its plan, stops its helpers the gentle
  way, as **Pause** does.
- Pausing doesn't stop the main agent's current turn (press Esc for
  that), but the agent can't start a task until you resume.
- A helper that finishes before its next step still reports, and its
  change comes to the apply card as usual.

## When a plan is done

When the last open task of a building plan is done or not needed, the
plan is **Done** by itself. Its banner sums up what was verified: tasks
done, criteria met by checks, by inspection or by you, the checks that
passed, and anything waived. **Reopen** makes it building again; an
abandoned plan's banner has **Reopen** too.

## After a restart, or when a chat opens

When a chat with a plan opens (also after HyperForm restarts), or you
attach a plan to a chat, HyperForm checks where the work stands:

- A task whose helper is gone waits again, with a note on where it
  stopped. Its worktree is kept (its detail then says "Interrupted; its
  helper's work is kept in a worktree", with **Show worktree** and **Its
  helper's transcript**), and the interrupted start doesn't count toward
  `work.max_attempts`.
- A change still waiting at the chat's apply card keeps waiting.
- Files changed since the plan last saw your project are checked against
  the plan: a deleted file a task relies on, or a done task whose files
  changed, sends the plan to **Replanning**, holding only the tasks it
  affects ([how](18-work-plans.md#what-changes-for-the-agent)).
- The transcript shows a quiet **Plan resumed** row, such as "2 tasks were
  interrupted, 1 file changed outside the plan". When nothing changed,
  nothing is added.
- The agent's next turn starts from where the plan stands (what is done,
  what is current, known problems, the next step) and its task packet, so
  it continues instead of starting over. It can continue an interrupted
  helper, with that helper's earlier work and worktree, or start a fresh
  one (the old work is then dropped).

The note says why the task stopped:

| Note | When |
|---|---|
| "its helper stopped before it finished" | The helper was gone when the chat opened (a restart or a crash). |
| "its helper finished, but its change was never applied" | The helper ended, but its change no longer waits at an apply card. |
| "the main agent of another chat had started it" | Another chat's main agent was on it. |
| "the plan was paused" / "the user held the task" | You paused the plan or held the task. |
| "the chat closed" / "the plan was detached from its chat" / "the plan moved to another chat" | Its chat closed, let the plan go, or lost it to another chat. |

HyperForm notes where the plan last saw your files after each of the main
agent's turns and after each helper change you apply, so it can tell what
changed outside the plan.

## Continue a plan in this chat

1. Open **Continue a plan**: it is in the empty **Plan** tab, the
   composer's **+** menu (**Continue a plan…**), the palette (**Continue
   plan: *goal***, or **Continue a plan…** for the full list) and the
   plan menu (**Switch to another plan…**).
2. Pick a plan. Each row shows its goal, status, "*n* of *m* done", what
   needs you ("2 things need you") and when it last changed; done and
   abandoned plans are under **Done and abandoned**.
3. The plan is attached to this chat, replacing the one it had (whose
   helpers stop), and the **Plan** tab shows it. A paused plan stays
   paused until you click **Resume**.

One chat drives a plan at a time. If the plan is attached to another
chat, you confirm **Move it here** first. That chat keeps its messages but
loses the plan, says "This plan moved to another chat.", and its helpers
stop (at once if it is open, otherwise the next time it opens).

The **Plan** tab remembers each plan's opened rows, selected task,
notebook tab and scroll position.

## From the project home and the Inbox

- The [project home](02-everyday-use.md#the-project-home)'s **Continue**
  card shows the most recent plan that is building, paused or replanning,
  with its progress and what needs you. **Continue** opens the chat it was
  last in on its **Plan** tab; if that chat is gone, a new chat takes the
  plan and is told "Continue the plan." A paused plan is only attached:
  nothing restarts until you click **Resume**.
- The [Inbox](02-everyday-use.md#the-inbox)'s **Needs you** has one item
  per plan that waits on you ("3 things need you"); **Open plan** opens
  its chat on the **Plan** tab, or a new chat that takes the plan if that
  chat is gone.

## What needs you

The **Needs you** strip in the **Plan** tab, the plan list, the home card
and the Inbox count the same things:

- a merge conflict;
- a helper's change waiting to be applied;
- a helper that ended without a report, or whose change you discarded;
- a proposed task, requirement or invariant;
- a **Verifying** task whose checks need to run again;
- an open audit finding of **High** severity.

Medium and low findings stay in the plan's findings list (click the
coverage line in the plan's header). The strip also notes the tasks that
wait for you to accept a plan change.

See also: [Work plans](18-work-plans.md) ·
[Plan mode with work plans](05-plan-mode-questions-and-todos.md#plan-mode-with-work-plans) ·
[Helpers on plan tasks](04-agents.md#helpers-on-plan-tasks) ·
[How pausing and resuming work](../how-it-works/features-work-resume.md) ·
[Troubleshooting](14-troubleshooting.md#a-work-plan-refuses-a-change)
