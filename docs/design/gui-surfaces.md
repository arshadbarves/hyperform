# HyperForm GUI — Surfaces

Status: **canonical** · the companion to the [UI guide](gui-ui-guide.md).
It says which component and which pure function own each surface of the
app: the transcript and composer, the one home of every fact, the side
panel and overlays, and the pet. Keep it true when a surface moves.

## 1. Chat surfaces

- **Transcript.** Turns from `lib/domain/sessionView` and `lib/domain/timeline`:
  user cards (Copy), streamed text with a thinking disclosure, tool calls and
  a one-line receipt. `workSection()` (`timeline/groups.ts`) gathers a
  turn's thinking and tool calls into one work block ahead of its answer;
  once the turn is done and made two or more calls, `WorkSummary` folds it
  into one line (`workLine()`: "Worked for 1m 12s · read 6 files · ran 2
  commands", plus "· 2 tasks done" for work-plan tasks finished in the turn,
  `tasksDoneIn()` in `work/progress.ts`), with failed calls, agents, questions, plans, errors and
  compaction still in view. Inside, two or more consecutive calls fold into
  one run (`groupTurnItems()`, `summarizeRun()`; `ToolGroup`); `Agent`,
  `AskUserQuestion` and `ExitPlanMode` (`SOLO`) never fold. Edit, Write and
  Bash cards open only on failure; inline diffs reuse `DiffRows`. A fenced
  block is a `CodeBlock` whose colors arrive once it nears the screen and
  its reply has finished streaming.
- **Turn actions.** `TurnActions`, once at the end of a turn beside
  `TurnFooter`: Copy the answer (`answerText()`), Open changes (the Changes
  tab) and `RewindMenu` (Rewind to before this prompt). Compaction dividers,
  task-view dividers and plan trace rows that end a turn sit after them
  (`splitTrailingCompactions()`).
- **Receipt.** `TurnFooter` follows `ui.task_report_view` via
  `receiptPlan()` (`lib/domain/receipt.ts`): `quiet` (default) shows only a
  verdict that says something, `compact` adds file chips and duration ·
  cost, `detailed` adds tokens, Not applicable and open checks.
- **Long chats.** `Transcript` renders a window of turns
  (`timeline/window.ts`): the newest 30, 20 more each time the top comes into
  view, and at once back to a turn you jump to; only the newest 6 render in
  full, the rest fill in through `whenVisible()` as they near the screen.
  `ChatTimeline` is the prompt rail: a dot per prompt (two prompts or more,
  at most 40 spread by `sampleEvenly()`); a click jumps there.
- **Compaction.** While `view.compacting` (from `compactionStarted` until
  its `compacted` marker, a new main-agent reply, the turn's end or idle)
  the transcript ends in a "Compacting context…" row. Each marker is a
  divider in `LocalCards`, "Context compacted · 180k → 24k · Summary", whose
  disclosure holds the summary; with a work plan it adds "· plan restored"
  when the main agent was sent its task packet again after it
  (`restoreTimes()`, `planRestored()` in `work/trace.ts`).
- **Plan trace** (with a work plan attached): quiet one-line rows in
  `LocalCards` (`chat/PlanTraceRow`, from `planTraces()` in
  `work/trace.ts`), placed by time like dividers: "Plan approved · rev N",
  "Plan revised · rev N" with its reason and **Why** (the notebook's Changes
  entry), "Plan resumed" with what the resume found (tasks interrupted,
  files changed outside the plan; from the `resume` plan change), and a
  task that failed or is blocked with **Open** (its detail in the Plan
  tab). Approvals show only when seen live; the rest come back with the
  plan. The chat a plan moved away from keeps a "This plan moved to another
  chat." row from the engine's notice (`movedTraces()`), until the app
  restarts.
- **Decision surfaces** (experimental features, all quiet one-liners):
  `RouteChip` ("Routed" or "Subagent routed" · effort · model · reason,
  from `routeChosen`) and `TaskViewDivider` ("N earlier exchanges (X
  tokens) set aside for this task", with **Include full history** on the
  latest; from `taskViewApplied`) sit in `LocalCards`; the claim badge
  "Claimed, not checked" is a warn note in `TurnFooter` (`claimNote()` in
  `lib/domain/verification.ts`); `planning/Suggestions` ends the transcript
  with `PlanSuggestion`, `RuleSuggestion` and `ReviewSuggestion`
  (`currentSuggestions()` in `sessionView/suggestions.ts`: the current
  turn's cards, at most five kept, plan first only in Default mode with
  plan mode off). Only
  task views survive a reopen.
- **Approvals and questions** wait in the composer, not the transcript
  (`planning/PendingInteractions` inside `Composer`): the oldest approval
  (focused) or else the oldest question takes the draft's place, the draft
  is kept, and "N more waiting after this" counts the rest. `ApprovalCard`
  is phrased by `approvalQuestion()` (`lib/domain/approvals.ts`): Allow
  once, Always allow… (a `Menu`: this chat or this project), Deny… with
  feedback; y / s / p / n while focused. A plan waiting for review is a
  "Plan ready · Review" row in the transcript (`PlanReady`), which opens the
  Plan tab. Every waiting card or button (and the trust banner) has
  `data-pending-card`, which the island's button scrolls to and focuses.
- **Composer.** One floating glass surface (`Composer` with
  `ComposerInput`, `ComposerBar`, `ComposerPlusMenu`, `ComposerDropZone`,
  `ComposerQueue`, `ComposerAttachments`). Plan mode and the permission
  level are separate chips: `PlanChip` shows only while planning, and its
  **×** turns planning off (turning it on is the **+** menu's **Plan
  first**, Shift+Tab or the palette); `ModePicker` lists the three levels
  (Ask, Auto-accept edits, Bypass behind its warning confirm), both from
  `lib/domain/modes.ts`. The permission and model chips
  are `Popover`s, the model one with an `EffortRow` for reasoning models. Send
  and Stop are one button whose glyph swaps (Stop while a turn runs and
  the draft is empty).

## 2. Calm by default, one home per fact

**Calm by default.** At rest a surface says one line; the detail waits
behind a `Disclosure` or a click. Never fold away failures or anything
waiting on the user: failed steps show their last lines and cards open,
error notices open the island card, outcome notes always show, and
approvals, questions, plans and the trust banner stay in view and counted.

**One home per fact.** A summary may link to its home, and the ⌘K palette
and shortcuts may repeat any action, but two views never show the same fact.

| Fact | Home |
|---|---|
| Live status: step, retry, result, passing notice | the island (`Island`: `IslandCapsule`, `IslandCard`) |
| How full the context is | the context satellite (`ContextBubble`, `ContextCard`) |
| Other chats that need you | the waiting satellite (`WaitingBubble`), then the Inbox |
| Notice history, turns that finished in the background | the Inbox (`components/inbox/`) |
| Which of this project's work plans wait on you | the Inbox's Needs you list: one item per plan with anything waiting ("N things need you", `planInboxItems()`), opening the plan's chat and its Plan tab; the detail stays in that Plan tab's Needs you strip |
| The work plan to pick up next | the home Continue card (`ContinuePlan`): the most recent Building, Paused or Replanning plan with its progress and needs-you count; any other plan through Continue a plan (the Plan tab's Empty state, the composer's + menu, the palette) |
| What changed | the side panel's Changes tab (`ChangesButton` counts this chat's files) |
| The plan and its progress; with work plans, its tasks, their criteria, evidence and results, and the notebook | the side panel's Plan tab (`PlanView`) |
| What a work plan waits on you for: merge conflicts, changes ready to apply, tasks that stopped (no report, change discarded), tasks in Verifying whose checks did not run or are stale (Run checks again), proposed requirements, invariants and tasks, open high-severity audit findings (medium and low ones stay in the findings list), tasks held until you accept a plan change | the Plan tab's Needs you strip (`PlanNeedsYou`); the Apply button itself stays on the Agents tab's `ApplyCard` |
| Whether a work plan is paused, done or abandoned, and what to do next | the Plan tab's state banner (`PlanStateBanner`) under the header |
| Coverage and the audit's findings | the header's coverage line, which opens the findings list (`PlanCoverage`) |
| Which plan tasks run now, and who runs them | the Plan tab's Now lanes (`PlanNow`), linking each helper to its Agents tab row |
| Helpers and background jobs | the side panel's Agents tab (`WorkPanel`) |
| What the model was sent; with work plans, the task packet each agent was given | the side panel's Context tab (`PromptInspector`, `InspectorPacket`) |

- The island's state is the pure `liveStatus()` in `lib/domain/liveStatus.ts`,
  highest priority first: this chat needs you, a passing notice, provider
  retry, work in progress, a turn that just ended, the away recap, idle.
  With a work plan, work in progress reads the current task's step (the
  activity becomes its detail) and carries tasks done of total.
  `islandMode()` (`lib/domain/island.ts`) picks its shape: `rest` (pet and
  title), `live` (a step, retry, notice or result with its clock),
  `alert` (this chat needs you, with one Answer / Review / Approve button
  from `islandAction()` that focuses the waiting card) or `expanded` (the
  card). `islandLine()` makes it one line and at most one number;
  `noticeOpensIsland()` decides when a notice opens the card. Add a state
  there, with a test, rather than a new pill or banner.
- **The island card** (`IslandCard`, in the island's own surface, not a
  popover) summarizes and links to the homes above: the notice and its
  actions, Plan (done of total and the current step; with a work plan,
  tasks done of total and the current task's step, `islandPlanRow()`), the latest steps,
  Agents, Context, Waiting, Cost (this turn and the chat) and the last
  warnings. It closes on a click outside or Esc, which it takes before the
  side panel does.
- **The pet** shows the same state as body language (section 4).
- Toasts (`pushToast`) show on the island and stay in the Inbox: push one
  only for what the user cannot see; confirm visible actions at the control.
- Sidebar rows carry at most one mark (`sidebarMark()`); the open chat shows
  none, as the island names it. Sidebar controls (New chat, Search,
  Settings) join the title bar only while the sidebar is hidden.

## 3. The side panel and overlays

The side panel (`sidepanel/SidePanel`, `SidePanelTabs`; `sidepanel.css`) is
one glass card beside the stage with a tab band: Changes, Plan, Agents,
Context (`panelTabs()` in `lib/domain/sidePanel.ts`; a hidden `browser` tab
is reserved). Its state is `ui.panel { tab, open, expanded, width }`:
`openPanel(tab, target?, scope?)` opens a tab (Changes at a file and
scope), `togglePanel(tab?)` backs ⌘D, the Changes button and the panel
toggle. It resizes from its edge handle between 340 and 1100 px, always
leaving the stage 400 px (`clampPanelWidth()`; the width is kept on this
machine), or expands over the stage. `panelNudge()`, followed by
`followPanelNudges()`, opens the Plan tab for a newly pending plan and marks
Agents with a dot for a new helper; switching chats is not news. Tab labels
hide when the panel is 420 px wide or less; the empty span in the tab band
is the pet's `panel` perch.

| Surface | Components | Logic (`lib/domain/` unless noted) |
|---|---|---|
| Changes tab | `overlays/DiffPanel` with `DiffHeader`, `DiffFileTree`, `DiffView`, `DiffRows`, `DiffCode`, `DiffFileBar`; under 640 px wide it drops the layout switch and slims the file list, under 560 px it stacks the list above the diff (`diff.css`) | `foldContext()`, `splitRows()` in `diffRows.ts`; `highlightLine()` in `lib/highlight.ts` |
| Plan tab | `planning/PlanView` picks the state; `PlanReady` is its row in the transcript. Work plans off: `PlanDocument` (review, edit, approve), as before. On: Empty (`PlanEmpty`: Start planning, and below it `PlanPicker`, "Continue a plan", when the project has other plans) until a plan is attached, then `PlanHeader` (status word, revision, goal, progress, the plan's menu: Pause, Request replanning, Stop and Abandon, both confirmed, Switch to another plan…, Detach; the coverage line, Tasks \| Document), `PlanStateBanner` (a calm banner under the header: Paused with Resume, and Stop while helpers still hold tasks; Done with what was verified, task counts and Reopen; Abandoned, read-only, with Reopen), `PlanNeedsYou` (amber strip under the header, only while the plan waits on you and the plan is editable; most urgent first; of the audit findings only open high-severity ones, as the plan's needs-you count; nothing it lists folds, except findings past the first two, which become a count that opens the findings), `PlanNow` while building or replanning (one lane per running task: the main agent's first, then each task-bound helper with its sprite, status, step and worktree tag; the helper's tag opens its Agents tab row), and while paused as Stopping (each helper stops at its next step; its worktree is kept), `TaskTree` / `TaskRow` (a WAI-ARIA tree; failed, blocked and waiting-on-you rows never fold away; a quiet "on hold" or "waits for the plan change" badge), `TaskDetail` (Hold / Release in its menu) with `TaskNotices` (on hold, waits for the plan change, or interrupted with its resume pointer: Show worktree and the helper's transcript) and `TaskResult` (summary, observed files and checks before the helper's claims, then what it learned, marked from web after research; how its change came back, Retry serially and Show worktree after a conflict, the attempt count), `TaskCriteria` and `TaskWhy`, `PlanCoverage` / `FindingRow` (the findings drill-in from the coverage line, amber only while a high-severity finding is open: open by severity with Add to plan / Dismiss, then added, dismissed and resolved behind a disclosure, and Run audit), `PlanNotebook` / `NotebookEntry` (Confirm / Dismiss for unconfirmed entries) and a footer by state: `PlanFooter` (Draft: Build, its menu Build here / Build in a new chat / Build with helpers in parallel, Keep planning with feedback) or `PlanChangeSet` (Replanning: Accept changes / Keep planning, and the tasks held until you accept). `PlanPicker` lists the project's other plans newest first (goal, status word, n of m done, needs-you count, updated time; Done and Abandoned behind a disclosure); choosing one attaches it here, after a confirm only when another chat has it attached now (`attachedTo`), which loses it; a plan detached from its last chat attaches without one. The same picker replaces the attached plan from Switch to another plan… or the composer's + menu (Back or Esc returns). With `work_parallel` effective (it needs `work_plans` too), rows carry a quiet badge (can run beside others, or why it runs alone), the Draft's Build menu offers the parallel build once two tasks can run together, and the building plan's menu offers Allow helpers in parallel or Run one task at a time; with it off, none of these show. Over 720 px or expanded, the detail (or the findings) sits beside the tree; narrower it drills in and Esc steps back, out of a task first, then out of the findings | `panelPlan()` in `plans.ts`; `planPanel()`, `planActions()`, `wideLayout()`, `stepBack()` in `work/panel.ts`; `visibleRows()`, `treeKeyAction()` in `work/tree.ts`; `taskLook()`, `taskActions()`, `blockedText()`, `blockKind()` in `work/status.ts`; `needsYouItems()`, `needsStrip()`, `needLine()` in `work/needsYou.ts`; `nowLanes()`, `laneStep()`, `helperTask()` in `work/now.ts`; `coverageLine()`, `findingGroups()`, `findingRefs()` in `work/health.ts`; `resultView()`, `integrationLine()`, `attemptLine()` in `work/report.ts`; `workParallelOn()`, `parallelBadges()`, `parallelCount()` in `work/parallel.ts` over the `work_parallel` query (`parallelSet()` in `lib/runtime/work.ts`); `criterionLines()` in `work/criteria.ts`; `notebook()`, `entryLine()` in `work/notebook.ts`; `changeSet()` in `work/changes.ts`; `whyLines()` in `work/why.ts`; `stateBanner()`, `doneLines()` in `work/planState.ts`; `holdEdit()`, `holdBadges()`, `replanHeldTasks()`, `replanWaitLine()` in `work/held.ts`; `resumeNote()`, `resumeSummary()` in `work/resume.ts`; `laneStopLine()` in `work/now.ts`; `planPicker()`, `canPickPlan()`, `moveNotice()` in `work/picker.ts` with `summaryStatus()`, `summaryProgress()` in `work/summary.ts` over the `work_plans` query (`projectPlans` in `lib/runtime/plans.svelte.ts`, refreshed on `workUpdated` and on open); per-plan view state (expanded rows, selected task, notebook tab, scroll by `scrollPlace()`) in `ui.plan`; edits through `editWork` (`lib/runtime/actions.ts`), `buildPlan`, `attachPlanHere`, `takePlanInNewChat` (`lib/runtime/planFlow.ts`); `pickPlan()`, `showPlanPicker()` in `lib/stores/plan-actions.ts` |
| Agents tab | `agents/WorkPanel`, `ApplyCard`, `InterruptedCard`, `AgentRow`, `JobRow`; `AgentSummary` (with `pet/PetSprite`) is the one helper layout for its rows and the transcript's Agent card, and names the plan task a helper was sent for (it opens that task in the Plan tab). Under Ready to apply, an Interrupted section holds the worktrees kept for a plan task whose helper was interrupted (the task's resume pointer names it and the task does not wait on its apply): "Interrupted · kept for task “…”" (it opens the task), the resume note, how the task picks it up (resume or reopen the plan, or when the task runs again), and Discard only, after a confirm; no Apply, and it is not counted as ready to apply. The task's Show worktree stays | `agentSections()`, `workCounts()`, `agentActivity()` in `agentTree.ts`; `interruptedHelper()`, `awaitsApply()` in `work/interrupted.ts`; `helperTask()` in `work/now.ts` |
| Context tab | `overlays/PromptInspector` with `InspectorMap` (a ring of the context window whose legend filters the outline), `InspectorOutline`, `InspectorInsights`, `InspectorPacket` (with a work plan: the latest task packet per agent, or the task a detail asked for, from the `work_packet` query), `InspectorDecisions` (while a decision feature runs or has run); `InspectorReader` while expanded | `outlineGroups()` in `inspectOutline.ts`; `latestPackets()`, `packetSections()` in `work/packet.ts`; `decisionFacts()`, `decisionLine()` in `decisionTrace.ts` |
| Settings (full-window page) | `settings/SettingsPage`, `SettingsNav`, `ScopeMenu`, `SettingsGroup`; the Experimental page is `ExperimentalTab` with `DecisionModelCard` (runtime, connection, Test connection) and its `DecisionNativeModel` row (download, cancel, resume, remove) | `SETTINGS_SECTIONS`, `searchSettings()` in `settings/searchIndex.ts`; `foldGroups()` in `components/settings/folding.ts`; `listedFeatures()`, `featureEntries()` in `settings/features.ts`; card wording in `settings/decisionModel.ts` and `settings/nativeModel.ts` |
| Palette | `overlays/CommandPalette`; with work plans, a Plans group (Continue plan: goal, per open plan, and Continue a plan…) | `paletteActions()`, `workPlanItems()` in `lib/paletteActions.ts`; `rankPalette()` in `palette.ts` |
| Worktree dialog, shell drawer | `overlays/WorktreeDialog`, `overlays/ShellOverlay` (inside `Composer`) | `create_worktree`; `lib/shellStore.ts` |
| Home, Inbox, first run | `home/`, `inbox/`, `onboarding/`. With work plans, the home Continue card leads with `ContinuePlan` (status, needs-you, goal, progress, Continue: opens the plan's last chat if it still exists, else a new chat that attaches the plan and sends "Continue the plan.", or only attaches a paused plan; then the Plan tab), and the Inbox's Needs you list adds one Open plan item per plan that waits on you; with them off, both are as before | `startersFor()`, `setupChecklist()`; `continuePlan()`, `continueTarget()` in `work/homeCard.ts`, `openWorkPlan()` in `lib/stores/plan-actions.ts`; `inboxSnapshot()`, `planInboxItems()` in `work/inboxItems.ts`; `needsOnboarding()`, `onboardingPose()` |

## 4. The pet

- `pet/IslandPet` fills the island's slot; `pet/PetLayer` (in `AppShell`)
  draws it while it roams, click-through except the pet, below every
  popover. Spots register with `use:perch` (`lib/ui/perch.svelte.ts`):
  slots `island`, `hero` (home), `empty` (the Inbox's `EmptyState` art),
  `panel` (the side panel's tab band); edges `composer`, `sidebar` (footer).
  While the side panel is expanded the stage's perches (`STAGE_PERCHES`:
  hero, empty, composer) are out of reach. Only `petBehavior()` in
  `lib/domain/pet/behavior.ts` moves it: a new spot is a `PerchId` and
  `PERCH_SIZE` in `perches.ts` plus a rule and a test.
- Perches are measured in a frame callback after one appears, goes or
  resizes, on window resize and scroll, and on the live clock's tick (twice
  a second while a turn runs or the pet is lively). `petMotion.svelte.ts`
  runs a frame loop only while the pet hops, walks, turns, is thrown, drops
  or swings from your grip. The 3D pet draws through `petFrames`
  (`lib/pet3d/scheduler.ts`), one `requestAnimationFrame` loop for every
  view that runs only while one wants frames (60 fps for moves, blinks and
  morphs, 30 fps for mood loops and helpers), stops while the document is
  hidden, and draws once per change under Reduce Motion. Breathing is a
  CSS loop on the HTML `.pet-breath` wrapper, so an idle pet costs no
  frames.
- Every screen draws `pet/Pet`: the 3D `Pet3D` once `loadPet3D()`
  (`petView.ts`) has loaded it and `lib/pet3d` in their own chunk, else the
  SVG `PetFlat` (while loading, without WebGL, and while `webgl.lost`).
  `lib/pet3d/` is the only code that imports `three`; its one offscreen
  `WebGLRenderer` draws every view and copies it into the view's own 2D
  canvas. The boot splash and `PetSprite` stay SVG. A new prop or
  accessory needs its SVG part and its model (`lib/pet3d/props.ts`,
  `accessories.ts`); a new body motion its CSS keyframes and its entry in
  `lib/domain/pet/rig3d.ts`; face shapes live once, in `faceShapes.ts`.
- While the pet roams, the island's slot shows `pet/IslandPortrait`, so
  the island keeps its width: the pet's head (`framing="portrait"`),
  turned toward the roaming pet (`petUi.at`, published by `PetLayer`) by
  `portraitLook()` in `lib/domain/pet/portrait.ts`, in a ring of the
  status tone; still under Reduce Motion. The slot, the ring and the Off
  dot are in `styles/island-pet.css`.
- `ui.companion`: lively reacts to the agent and the user and roams while
  `ui.pet.roam`; calm reacts only to the agent and stays in the island;
  off shows the island's dot. Reactions (`petUi.react`: a boop, a level-up
  that `PetLayer` plays once the island is not asking for you) go through
  `reactionPose()` for both the island's pet and the roaming one.
- It only echoes `liveStatus()` (the tone tints its aura, the look its
  body; `aria-hidden`): never the only carrier of a fact, never
  interrupting. Under Reduce Motion its springs jump, it neither walks nor
  blinks, and `petIdle` plays no strolls or tricks. Only
  `lib/runtime/pet.svelte.ts` loads and saves `pet.json`
  ([how the pet works](../how-it-works/features-desktop-pet.md)).
