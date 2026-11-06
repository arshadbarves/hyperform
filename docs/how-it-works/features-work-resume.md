# Pausing, resuming and moving plans

How the experimental `work_plans` feature stops and restarts the work on
a plan: pausing, holding and stopping; resuming a plan after a restart or
an attach; one chat driving a plan; and the plan list, the home card, the
Inbox and what "needs you" counts. How to use it:
[Pausing and continuing plans](../user-guide/19-pausing-and-continuing-plans.md).
The plan itself: [Work plans](features-work-plans.md); helpers and
worktrees: [Parallel tasks and the plan audit](features-work-parallel.md).
Terms are in the [glossary](glossary-experimental.md).

## Pause, hold and stop

**In plain words.** Pausing a job sends the crew home once each of them
finishes the step in hand. Each leaves a note on the job card saying where
they stopped, and leaves their tools on site. Holding does the same for one
card. Stopping calls everyone off at once, mid-step.

**How it works**
1. **Pause** (from Building or Replanning) moves the plan to Paused, so
   no task can start ("the plan is paused"). Each helper on one of its
   tasks gets a *soft stop*: it ends at its next round boundary (calls in
   flight finish), or at once while it waits on an approval. Its task goes
   back to Pending with a *resume pointer*: the chat and agent it ran in,
   its worktree when its change sits in one (pending or conflicted), and
   a note ("the plan was paused"). **Resume** makes the plan Active again.
2. **Hold** (`holdTask`) soft-stops the helper on that task only ("the
   user held the task"); a held task doesn't start until **Release**
   (`releaseTask`). After a pause or a hold, the main agent's next turn is
   told which helpers were stopped.
3. Closing a chat soft-stops all of its helpers ("the chat closed").
   Detaching its plan ("the plan was detached from its chat") or moving
   it to another chat ("the plan moved to another chat") soft-stops the
   helpers on that plan.
4. A helper that completes before its next round reports as usual. An
   interrupted start doesn't count against `work.max_attempts`.
5. **Stop** cancels every helper of the plan at once, puts the tasks they
   held back to Pending without a pointer, and leaves the plan Paused.
   **Abandon** cancels them the same way and makes the plan read-only.
6. A change that closes the last open task of an Active plan also makes
   it **Done**. **Reopen** makes a Done or Abandoned plan Active again.
7. Pause doesn't stop the main agent's own turn (Esc does), but its next
   `TaskUpdate` to start a task is refused while the plan is paused.

**For developers**
- Edits: `WorkEdit::{Pause, Resume, Stop, Abandon, Reopen, HoldTask,
  ReleaseTask}` ([`edit.rs`](../../crates/z-engine-protocol/src/work/edit.rs))
  become proposals in `z-engine-work/src/proposal.rs`; `Stop` is
  `Proposal::StopPlan` (`kernel/plan_ops.rs`), Done is
  `propagate::finished`.
- Stopping helpers: [`work/interrupt.rs`](../../crates/z-engine-engine/src/work/interrupt.rs)
  (`halt_for` picks the helpers an edit stops; `settle_halted` runs when
  one ends) and [`work/bound.rs`](../../crates/z-engine-engine/src/work/bound.rs)
  (`BoundHelpers`; `Halt::Interrupt(note)` is a soft stop,
  `Halt::Stopped` a cancel). A soft stop is `AgentTracker::request_stop`
  (`orchestration/tracker.rs`), checked at each round in `run/agent.rs`.
- `Proposal::InterruptTask` (engine only, `kernel/run_ops.rs`) records
  `taskTransitioned` to Pending, `taskReleased` and
  `WorkEvent::TaskInterrupted { taskId, resume }`; `state/run.rs` folds
  `Task.resume` and `run.forgiven_attempts`. [`work/kept.rs`](../../crates/z-engine-engine/src/work/kept.rs)
  refuses applying such a helper's worktree by hand (`foreign_apply`)
  and, on Discard, submits `Proposal::DiscardResumeWorktree`, recorded as
  `resumeWorktreeDiscarded`: the pointer keeps its agent and note, and
  the next run starts clean.

## Resuming a plan

**In plain words.** When the crew comes back, a foreman walks the site
first: whose work was left half-done, what changed while everyone was
away, and where things stand. The crew then picks up from the notes
instead of starting over.

**How it works**
1. Resuming runs when a chat opens with its plan (after an app restart
   too) and when a chat attaches a plan, unless the plan is Done or
   Abandoned.
2. **Interrupted tasks.** Every Active task whose owner is gone goes back
   to Pending with a resume pointer: a helper that no longer runs ("its
   helper stopped before it finished"), one whose change no longer waits
   at this chat's apply card ("its helper finished, but its change was
   never applied"), or another chat's main agent ("the main agent of
   another chat had started it"). This chat's main agent keeps its task,
   and so do this chat's helpers that still run or whose change still
   waits at its apply card. A pointer keeps its worktree only while that
   folder exists.
3. **Outside changes.** The plan remembers the tree it last saw: after
   each main-agent turn and each helper change that reached the project
   tree, when the tree changed, the engine links the workspace
   [fingerprint](glossary.md#fingerprint) and a shadow checkpoint
   (`checkpointLinked`). On resume, a different fingerprint means files
   changed outside the plan. The files changed since the last linked
   checkpoint are diffed and [reconciled](features-work-plans.md#reconciliation-and-replan):
   a deleted scope file or a done task whose files changed becomes a
   finding, which holds only the tasks it affects. The current tree is
   then linked.
4. **One plan change.** When something was interrupted or changed, one
   `PlanChange` with cause `resume` is recorded, such as "Resumed: 2 tasks
   were interrupted, 1 file changed outside the plan", with the changed
   files as diff evidence. The transcript shows it as a quiet **Plan
   resumed** row. A reopen with nothing new records nothing.
5. **The first turn.** The next turn starts with the resume reminder:
   the goal and status, what resuming found, the completed and current
   tasks (with their resume notes), known issues, open findings and the
   next ready task. Then comes the task packet, with reason `resume`.
6. **Continuing a helper.** `Agent` with `task_id` and `resume` may
   continue only the agent the task's pointer names, from the chat it ran
   in. That helper resumes with its transcript, in its own worktree. Any
   other is refused ("Task … cannot be resumed: …"). `Agent` with
   `task_id` alone may always start the task afresh, which drops the
   pointer.

**For developers**
- Kernel: [`z-engine-work/src/resume.rs`](../../crates/z-engine-work/src/resume.rs)
  (`interrupted`, `Alive`, the notes `STOPPED`, `UNAPPLIED`,
  `OTHER_MAIN`) and `dispatch::resume_check`.
- Engine: [`work/resume.rs`](../../crates/z-engine-engine/src/work/resume.rs)
  (`resume`, called from `attach::reattach` and `attach_command`;
  `remind` at the turn start) and
  [`work/checkpoint.rs`](../../crates/z-engine-engine/src/work/checkpoint.rs)
  (`link` after each turn and each applied change, `recorded_fingerprint`,
  `last_checkpoint`), with `facts::diff_since`. The causes are
  `WorkCause::Resume` and `ChangeCause::Resume`.
- The reminder: `z-engine-context/src/work/resume.rs` (`work_resume`,
  `ResumeBrief`) and `prompts/reminders/work-resume.md`. It is due after
  every resume, also when nothing was found.
- GUI: `lib/domain/work/resume.ts` (the task's note), `trace.ts` (**Plan
  resumed**), `interrupted.ts` (the Agents tab's "Interrupted · kept for
  task …") and `components/planning/TaskNotices.svelte`.
- A plan that never linked a checkpoint compares with the tree of its
  latest check evidence and has no diff base, so its first resume may say
  "the workspace changed outside the plan" once, without files.

## One chat drives a plan

**In plain words.** A job folder sits on one desk at a time. Taking it to
another desk leaves a note on the old one, and whoever was working from
the old desk stops.

**How it works**
1. The chat that attached a plan last drives it. `meta.json` keeps it as
   `lastSessionId` (kept after a detach) and the chat it is attached to
   now as `attachedTo` (a closed chat included; null after a detach).
2. Attaching a plan that another open chat holds moves it, whether from
   the picker, the home card, the Inbox or **Build in a new chat**. The
   other chat logs `planDetached`, its helpers on the plan soft-stop, and
   its window gets a snapshot without the plan and the notice "This plan
   moved to another chat."
3. A chat that was closed when its plan moved finds, when it next opens,
   that another chat drives the plan: it records `planDetached` instead
   of reattaching it.
4. The GUI asks before moving only when `attachedTo` names another chat
   that still exists. Attaching a paused plan only attaches it.

**For developers**
- [`work/driver.rs`](../../crates/z-engine-engine/src/work/driver.rs)
  (`Driver`, `drive`, `moved_away`, `may_reattach`, `MOVED_NOTICE`) and
  `work/attach.rs` (`attach_command`, `reattach`, `ensure`).
- GUI: `lib/runtime/planFlow.ts` (`attachPlanHere`, `takePlanInNewChat`)
  and `lib/stores/plan-actions.ts` (`pickPlan`, `openWorkPlan`,
  `showPlanPicker`).

## The plan list and what needs you

**In plain words.** Each job folder has a cover sheet: how far along it
is, how many things wait on you, and which desk last worked on it. The
home page, the Inbox and the plan list read the cover sheets instead of
opening every folder.

**How it works**
1. Each accepted change rewrites the plan's `meta.json` (`PlanSummary`):
   goal, status, revision, tasks total and done, `blocked`, `running`,
   `needsYou`, `lastSessionId`, `attachedTo` and its times.
2. `needsYou` counts merge conflicts, changes waiting to be applied,
   helpers that ended without a report or whose change was discarded,
   proposed tasks, requirements and invariants, Verifying tasks whose
   checks need to run again, and open audit findings of high severity.
   Medium and low findings stay in the findings list. The Plan tab's
   **Needs you** strip lists the same things from the live plan, plus
   the tasks a plan change holds.
3. The window keeps each project's plan list (the `work_plans` query) and
   refreshes it shortly after a plan changes. The picker lists open plans
   newest first, with the done and abandoned ones behind a disclosure.
   The home card shows the most recent Building, Paused or Replanning
   plan, and the Inbox shows one item per plan that has something
   needing you.
4. **Continue** on the home card opens the chat the plan was last in, or
   a new chat that attaches it and is sent "Continue the plan." (a paused
   plan is only attached). The Inbox opens the same chat, or a new chat
   without a message.
5. The Plan tab keeps each plan's view: its opened rows, the selected
   task, the notebook tab and the scroll position.
6. Whether work plans are on for the Inbox and the home is read from the
   active project's settings.

**For developers**
- Kernel: [`z-engine-work/src/needs.rs`](../../crates/z-engine-work/src/needs.rs)
  (`needs_you`, `needs_recheck`); the summary in `state/mod.rs`
  (`PlanState::summary`), written by `PlanHandle::save_meta`.
- GUI: `lib/runtime/plans.svelte.ts` (`projectPlans`), and in
  `lib/domain/work/`: `picker.ts`, `homeCard.ts`, `inboxItems.ts`,
  `summary.ts` and `needsYou.ts`. Components: `planning/PlanPicker.svelte`,
  `planning/PlanEmpty.svelte`, `planning/PlanStateBanner.svelte`,
  `home/ContinuePlan.svelte`, `inbox/InboxNeedsYou.svelte`. The per-plan
  view state is in `lib/stores/ui.svelte.ts`.

See also: [Work plans](features-work-plans.md) ·
[Parallel tasks and the plan audit](features-work-parallel.md) ·
[Desktop screens](features-desktop-screens.md) ·
[v2 engine: work plans](../architecture/v2-engine-work.md)
