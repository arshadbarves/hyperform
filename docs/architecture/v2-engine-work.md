# Engine architecture: work plans

The runtime contract for the experimental work plans (`work_plans`) and
parallel tasks (`work_parallel`): the kernel, plan storage, the inputs and
outputs a session wires to a plan, task-bound helpers, integration of
their changes, the plan audit, and pausing, resuming and moving plans.
Both features are Off or On (no Shadow); `work_parallel` requires
`work_plans` and stays Off without it.
The runtime, the tool gate, persistence, settings, hooks, commands and
context are in [v2 engine architecture](v2-engine.md).

## Kernel

The pure kernel `z-engine-work` decides every change: `decide(state,
submission, facts)` checks the actor's authority (user, kernel,
coordinator, worker, untrusted), the state machines, the dispatch and
parallel rules and the completion gate, and returns the records to append
or a typed rejection. It does no I/O. Every accepted revision also gets
the kernel's follow-ups: failure and supersession propagation
(`kernel/propagate.rs`), Done once a submission closes the last open task
of an Active plan (`propagate::finished`), and the deterministic audit
(`kernel/audit_ops.rs`, recorded only when its findings change).

## Storage

- **One writer per plan.** `PlanHub` (`work/hub.rs`, created once per
  `Engine`) keeps one live `PlanHandle` per plan for the whole process,
  shared by every session that attaches it and dropped when none does.
  `PlanHandle::submit` decides, appends and syncs the records, applies
  them, rewrites `meta.json` and emits `workUpdated` to every watching
  session; a rejection is appended as an audit record. A failed write
  makes the handle read-only until the plan is opened again.
- **Files.** `PlanStore` (`z-engine-store/src/plans/`) keeps
  `<data dir>/plans/<project key>/<plan id>/events.jsonl` (one
  `WorkRecord` per line, `PLAN_SCHEMA` 1) and `meta.json`; the project
  key is `z_engine_host::project_key`. The session log only records
  `PlanAttached { plan_id }` and `PlanDetached`, so replay reattaches.
- **Summary.** `meta.json` is the `PlanSummary` from
  `PlanState::summary`: goal, status, depth, revision, `tasks_total`,
  `tasks_done`, `blocked`, `running`, `needs_you` (`needs::needs_you`),
  and the engine's `last_session_id` (the driving chat, kept after a
  detach) and `attached_to` (`None` after a detach). New fields default
  when an older file is read. The `work_plans` query lists them.

## Inputs and outputs

- **Inputs.** The tools `TaskCreate`, `TaskUpdate` (with `report` for a
  task-bound helper), `TaskGet`, `TaskList` and `PlanRecord` reach the
  plan through `WorkPort`; `ExitPlanMode` outlines it before review and
  `PlanDecision::Approve { handoff }` builds it here or in a new chat. The
  GUI sends `editWork { plan_id, edit }`, `attachPlan { plan_id }` and
  `detachPlan`; the edits include `approveParallel`, `addFinding`,
  `dismissFinding`, `requestAudit`, `recheckTask`, `retrySerially`,
  `acceptTask`, `pause`, `resume`, `stop`, `abandon`, `reopen`,
  `holdTask` and `releaseTask` (`WorkEdit`, mapped to proposals in
  `z-engine-work/src/proposal.rs`). Recorded checks become evidence
  (`work::link_check`: on the agent's own active task and, for the main
  agent's checks, on every Verifying task with a check criterion of that
  kind) and `TodoWrite` lists become the active task's steps.
- **Outputs.** Each turn the engine pins the plan in the prompt and sends
  the current task's packet when it is due (`work/inject.rs`), emitting
  `taskPacketSent { agent_id, task_id, tokens, reason }` (reasons include
  `dispatch` and, for a helper after its own compaction, `compaction`).
  Compaction keeps the plan and restores it after the summary.
  Reconciliation (`work/reconcile.rs`) compares the plan with the
  workspace when its fingerprint changes, after a discovery or a helper's
  outcome, and on `/replan` (`work/replan.rs`). Changed files are taken
  since the last code checkpoint (`Facts.changed_since`); a Done task
  updated after it isn't compared, so its own edits never count as stale
  (an outside edit in the same turn right after it completes isn't
  flagged either). New findings move an Active plan to Replanning, as
  the Kernel actor, which holds only the tasks they affect
  ([held tasks](#held-tasks)); only the user's `approve` returns it to
  Active, and a standing finding isn't re-reported. A plan that resumes
  is reconciled too ([resume](#resume)).
- **Queries.** `work_plans`, `work_packet`, `work_why` and `work_parallel`
  (the `ParallelSet`: `preview` for a Draft, `eligible` otherwise) in
  `engine/queries/work.rs`. The snapshot and `workUpdated` carry `audit`
  and `health` (`PlanHealth`).

## Task-bound helpers

- **Dispatch.** `Agent { task_id }` exists only while `work_plans` is On
  (`AgentTool::with_tasks`); otherwise the field isn't in the schema and
  a `task_id` sent anyway is ignored, so an ordinary helper runs.
  `orchestration/task_run.rs::admit` hands a `resume` on, and
  `work/dispatch.rs::admit` refuses a non-main caller, a
  chat without a plan, a second task-bound helper unless `work_parallel`
  acts and the plan's policy is `Approved`, and more helpers than
  `Settings::max_parallel_tasks()` (`work.max_parallel` capped by
  `agents.max_concurrent`). The kernel's `dispatch::check` needs a
  Pending, Ready, unowned task with attempts left (`attempts -
  run.forgiven_attempts < max_attempts`) that passes `beside_running`;
  beside a running task the worktree is required, and once parallel work
  is approved it is always requested (`Admission::worktree_wanted`), so
  helpers dispatched in the same round can't race into the project tree.
  With `resume`, `dispatch::resume_check` also needs the task's resume
  pointer to name that agent in this session ("Task … cannot be resumed:
  …"); the resumed helper re-enters its worktree (`launch.rs`). Every
  dispatch, resumed or fresh, clears the pointer. `bind` records `TaskDispatched`
  (`TaskDispatch { session_id, agent_id, worktree, attempt, at }`); a
  helper that needed a worktree and got none fails before it runs.
- **Brief.** The helper's first message is `helper_brief(packet, prompt)`
  (`prompts/work/helper.md`): the packet and the result contract.
- **Worker role.** A bound helper may edit, set status on, set criteria,
  steps, evidence notes and dependencies of, and report on its own task
  only; it may create subtasks of it only as Proposed and record notes.
- **Scope drift.** In `batch/gate.rs`, before `review_call`,
  `work::outside_scope` matches a bound helper's `Edit`, `Write`,
  `MultiEdit` or attributable shell redirect against its task's scope; a
  hit is noted once per path as a kernel discovery ("helper for tN tried
  to write outside its scope: *path*") and, unless the mode
  is Bypass, `escalate` turns Allow into Ask with the drift reason (Deny
  stays).
- **Result.** A `TaskUpdate { report }` is held in the session
  (`save_report`) until the run ends, unless it comes with `completed`;
  a `completed` without one takes the saved report, and `conclude`
  records the saved report on every other ending.
  `settle` folds it with `RunFacts` (worktree files from `git diff
  --name-only base branch`, `None` when unknown; the helper's check
  records) in a `TaskReported` event; the helper's own files and tests
  become `ReportClaims`, and a web-reading run marks the report
  `Research`. No report: `ReportSource::Unreported` and
  `Blocked { Unreported }`.

## Integration

- **Policy.** After a report, a run with no worktree or an `Empty` one is
  `Integration::Applied` at once. Otherwise `work.merge` decides:
  `Ask` records `PendingApply` and leaves the apply card; `Auto` applies
  under the session's merge lock (`core.agents.merge_lock()`), and an
  apply error posts a warning and falls back to `PendingApply`.
  `restrict_to_user_level` drops an untrusted project's `Auto` and
  re-adds the user's `serial_paths`; `trust_gated` reports "automatic
  merges" and "fewer serial paths".
- **Verify in the project tree.** `Applied { files }` moves the task to
  Verifying and spawns `verify_merged`: each check kind the criteria name
  runs once in the project root as the helper's check, the records link
  as evidence, and the gate gives Done, Failed ("the integration check
  *label* failed in the project tree", propagated by the kernel), or
  leaves it Verifying. The user's `recheckTask` (`merged::recheck`) has
  the kernel confirm the task is Verifying (`Proposal::Recheck`), then
  runs `verify_merged` again.
- **Conflict and discard.** A conflicted apply leaves the tree untouched,
  keeps the worktree and records `Conflict { paths, worktree }` with
  `Blocked { MergeConflict }`; applying the kept worktree later counts as
  applied. A discard records `Discarded` and `Blocked { ChangeDiscarded }`.
  `RetrySerially` (user only) resets the run to Pending with `serial`
  and fresh attempts (`TaskRetried { serial, fresh_attempts }`), and
  `drop_kept` removes the old worktree.
- **Recovery.** On session open `orchestration/recover.rs` only runs
  `git worktree prune`; a task's worktree stays until it is applied,
  discarded or serially retried, and an interrupted task's stays for its
  resume pointer. Applying such a helper's worktree is refused
  (`work/kept.rs::foreign_apply`: "This helper's work belongs to task ….
  Resume the task or discard its work."); discarding it submits
  `Proposal::DiscardResumeWorktree` (kernel only, `kept_discarded`),
  recorded as `ResumeWorktreeDiscarded`, which clears the pointer's
  worktree.
- **Coordinator.** After every outcome `tell_coordinator`
  (`work/coordinate.rs`) sends the main agent `reminders/work-coordinate.md`
  (event, status, report summary, newly ready tasks, the parallel set,
  the tasks the replan holds; `News::Interrupted` after a pause or hold)
  and reconciles with `Trigger::Helper`.

## Parallel eligibility

`parallel::eligible` (the gate: plan mode off, policy `Approved`) and
`parallel::preview` (no gate, for a Draft) walk ready tasks in priority
and tree order and exclude, beside each running or chosen task:
`RunAlone` (`run_alone` or `run.serial`), `UnknownScope` (no files),
`SerialPaths` (a `work.serial_paths` match), `Ordered` (a dependency
path), `Overlap` (files or symbols), `Replanning` (a task the replan
holds), and `NoSlot` past `max_parallel`. `describe` gives each block's
refusal text.

## Plan audit

- **Deterministic** (`z-engine-work/src/audit.rs`): uncovered
  requirements, criteria whose check kind the project lacks, risky paths
  (`RISKY_PATHS` plus `work.sensitive_paths`) without a test criterion,
  open leaf tasks without criteria, risks without a mitigation, overlaps
  and duplicates. Ids are stable; `Dismissed` sticks, `Resolved` reopens
  when found again, and findings no longer found resolve.
- **The `audit` agent** (`prompts/agents/audit.md`, registered only while
  `work_plans` is On; spawned as a helper it has `Read`, `Glob` and
  `Grep`): `work/audit.rs` sends one tool-less `side_request` with the
  agent's prompt and model role, the plan as JSON (document 8,000 chars,
  40 entries, open findings) and `max_tokens` 4,096, parses at most 20
  findings (`ModelReview`, ids `review-<n>`, unknown refs dropped) and
  records them as `AuditSource::AuditAgent`. `review_deep_plan` runs it
  from `ExitPlanMode` when the plan's depth is Deep; the depth is first
  re-assessed from all open tasks (`reassess_depth` with `assess_plan`),
  as it is again on approval. `requestAudit` runs it on demand. Failures record nothing and post an Info notice.
- **Add to plan** (`work/findings.rs`) marks the finding `Added`, creates
  a user-authored Pending task from it and reminds the main agent
  (`reminders/work-finding-added.md`).
- **Health** (`z-engine-work/src/health.rs`) is computed from the state
  for every snapshot and update (`PlanState::snapshot`, `update`).

## Held tasks

A failure holds only what it affects. `PlanState.replan_by_findings` is
set when the Kernel actor moved the plan to Replanning (reconciliation,
or a resume that found problems) and cleared by any other move.
`held::partial` is that flag while Replanning; `held::replan_held` is
then every open task a `ReconcileFinding` names plus its transitive
dependents, and `held::holds` asks the same for one task through its
dependencies. Readiness gives such a task `Readiness::ReplanHeld` ("it is
held until the plan change is accepted", `HELD_FOR_REPLAN`); every other
Pending task is judged as in an Active plan, so dispatch, the parallel
set and packets go on. A held task is `ParallelBlock::Replanning` and its
packet carries the `Plan { Replanning }` blocker. Approval or the findings
resolving release them; no event records the hold. A User-actor move
(planning on, `RequestReplan`, `/replan` with only text), Draft and
Paused still hold every task through `PlanNotActive`. The snapshot and
`workUpdated` carry `replanHeld`.

## Pause, hold and stop

- **Kernel.** `Pause` moves Active or Replanning to Paused, `Resume`
  Paused to Active, `Reopen` Done or Abandoned to Active (all
  user-only). `Stop` is `Proposal::StopPlan` (`kernel/plan_ops.rs`):
  Paused, and every task bound to an agent released (Active tasks to
  Pending). `HoldTask { held }` / `ReleaseTask` set `Task.held`
  (`Readiness::Held`, "the user holds it").
- **Engine.** After an accepted edit, `interrupt::halt_for` picks the
  helpers to stop: `Pause` all of the plan's (`Halt::Interrupt(PAUSED)`),
  a hold the task's (`HELD`), `Stop` and `Abandon` all
  (`Halt::Stopped`). `BoundHelpers` (`work/bound.rs`) tracks each bound
  helper's plan, task, tracker and cancel token; a soft stop is
  `AgentTracker::request_stop`, read at the next round in `run/agent.rs`
  (a helper waiting on an approval is cancelled at once), and a hard stop
  cancels. Closing the session (`interrupt::closing`, `CLOSED`), detaching
  (`DETACHED`) and a move (`MOVED`) soft-stop the helpers concerned.
- **Settling.** When a bound run ends, `settle_halted` decides: a
  completed run settles as usual; a soft-stopped one submits
  `Proposal::InterruptTask` (engine only) with a `ResumePointer {
  session_id, agent_id, worktree, note }`, the worktree kept when it is
  Pending or Conflicted. The kernel records `TaskTransitioned` to
  Pending, `TaskReleased` and `TaskInterrupted`; the fold forgives the
  attempt. A hard-stopped run's report is dropped.

## Resume

`work::resume` runs from `attach::reattach` (a session opening with its
plan, before the first snapshot) and `attach_command`, except for a Done
or Abandoned plan. In one Kernel submission with `WorkCause::Resume`:

1. `z-engine-work/src/resume.rs::interrupted` returns every Active task
   whose owner is gone, given `Alive` (this session, its running helpers
   and those whose change waits at its apply card): `STOPPED`,
   `UNAPPLIED` or `OTHER_MAIN`. Pointers keep a worktree only if its
   folder exists. Each becomes `InterruptTask`.
2. Outside changes: if the workspace fingerprint differs from
   `checkpoint::recorded_fingerprint` (the last `CheckpointLinked`, else
   the newest check evidence), the files changed since the last linked
   checkpoint (`facts::diff_since`) feed `findings`; new ones add
   `Reconcile`.
3. `LinkCheckpoint` records the tree now, with a shadow snapshot.
4. When anything was interrupted or changed, one `PlanChange` entry with
   `ChangeCause::Resume` ("Resumed: 2 tasks were interrupted, 1 file
   changed outside the plan"), its tasks, findings and the diff as
   evidence.

Then `resume_due` makes the next turn start with `work_resume`
(`reminders/work-resume.md`) and the packet (reason `resume`).
`checkpoint::link` keeps the base current: after each main-agent turn
and each applied helper change, when the fingerprint moved, it records
`CheckpointLinked { fingerprint, checkpoint }`.

## The driving chat

`PlanHandle` carries a `Driver` (`work/driver.rs`): the session that
attached the plan last and whether it still holds it, read from
`meta.json` (`Driver::recorded`) and stamped into every summary
(`stamp`); a detach, or attaching another plan, clears the hold
(`released`), so `attached_to` turns `None`. `drive` runs on
every attach (`attach_command`, `reattach`, `ensure`): when another live
session held the plan, `moved_away` logs its `PlanDetached`, soft-stops
its helpers on the plan (`MOVED`), emits a snapshot without the plan and
the notice "This plan moved to another chat.". A session that reopens
while another drives its plan logs `PlanDetached` instead of reattaching
(`may_reattach`). The GUI confirms a move only when `attached_to` names
another existing chat (`picker.ts`).

## Plan events and edits added in milestone 3

- `WorkEvent::TaskInterrupted { task_id, resume }` and
  `ResumeWorktreeDiscarded { task_id }`; `Task.resume`; the engine-only
  proposals `InterruptTask` and `DiscardResumeWorktree`.
- `WorkEdit::ReleaseTask`; `ChangeCause::Resume`, `WorkCause::Resume`.
- `WorkSnapshot.replan_held` and `WorkUpdate.replan_held`;
  `ParallelBlock::Replanning`.
- `PlanSummary.blocked`, `needs_you`, `running`, `last_session_id`,
  `attached_to`.

## Architecture re-evaluation after milestone 3

**What held up.**
- *The kernel as the single writer.* Pausing, stopping, holding,
  interrupting, finishing, reopening and resuming are all proposals
  decided by `decide` and recorded as `WorkRecord`s; the engine only
  supplies facts. Resume is one Kernel submission, so it lands as one
  revision and one plan change.
- *Proposal into WorkRecord.* Every new behavior is a `Proposal`
  variant with an authority rule in `authority.rs` and a fold rule in
  `state/`; engine-only steps (`InterruptTask`,
  `DiscardResumeWorktree`) use the Kernel role, so no new trust path was
  needed.
- *PlanHub.* One live handle per plan made the driving chat a field on
  the handle, not a lock service; every watcher sees each change.
- *No new stores or event systems.* The project-level surfaces (the
  picker, the home card, the Inbox) read `PlanSummary` from `meta.json`;
  the GUI hears `workUpdated` and the session snapshot as before.
  Checkpoint links reuse the shadow-checkpoint repository.

**What was simplified.**
- The replan hold is derived from the state (one flag set by the
  record's actor, the findings and the graph), not a stored task status,
  so accepting or resolving releases it without new events.
- The soft stop reuses the round loop (`request_stop`) and the hard stop
  reuses cancellation; resuming a helper reuses `Agent { resume }` and
  worktree re-entry.
- Outside changes reuse reconciliation and the shadow checkpoints
  rather than a separate change tracker.
- "Needs you" is one kernel count (`needs::needs_you`) for the summary.

**Still open.**
- Shell `curl` and MCP tools aren't tracked by the web-reading rule.
- An outside edit right after a task's completion turn isn't flagged as
  stale evidence.
- A plan that never linked a checkpoint falls back to its latest check
  fingerprint and may show a false "changed outside the plan" once.
- The Inbox and home read the work-plans switch from the active
  project's settings, not each project's.
- The "Next" line under the Now lanes and the plan header's elapsed time
  aren't built; focus doesn't return after the narrow panel's back.
- Graduation evidence for `work_plans` and `work_parallel` (the
  long-task and parallel fixtures) doesn't exist yet
  ([status](../status.md#graduation-criteria)).

**Risks.**
- The Plan tab's **Needs you** strip (`needsYou.ts`) mirrors the
  kernel's count; the two can drift if one changes alone.
- `PlanHub` gives one writer per process; two app instances on the same
  data folder could both append to a plan log.
- The driving chat lives only in `meta.json`, which is rewritten after
  the log append; a crash between the two leaves the summary one change
  behind until it is next rewritten (an accepted change or an attach).
- A resume pointer names a session and an agent; continuing it relies
  on that session's subagent transcript and worktree still existing.

How it works in plain words: [work plans](../how-it-works/features-work-plans.md),
[parallel tasks and the plan audit](../how-it-works/features-work-parallel.md)
and [pausing, resuming and moving plans](../how-it-works/features-work-resume.md).

See also: [v2 engine architecture](v2-engine.md) ·
[servers, verification and sandbox](v2-engine-services.md) ·
[AGENTS.md](../../AGENTS.md)
