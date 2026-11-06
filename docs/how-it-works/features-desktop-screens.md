# The desktop app's screens

What each main surface of the window does, from the project home to the
palette. How the window talks to the engine, how events become the screen and
how the app updates itself are on [The desktop app](features-desktop-app.md);
the pet has its own page, [The desktop pet](features-desktop-pet.md).
Terms are in the [glossary](glossary.md); the UI rules in the
[GUI UI guide](../design/gui-ui-guide.md).

## The project home, the Inbox and first run

**In plain words.** With no chat open, a project opens on its home page, a
desk with today's list on it. The Inbox is the tray where whatever happened
while you looked elsewhere waits.

**How it works**
- The home names the project and branch, asks "What should we work on?"
  and offers starter prompts that fit the project, then cards shown only
  when they apply: **Continue**, **Uncommitted changes** and **Set up this
  project**. The first message turns the home into the chat. With work
  plans on, **Continue** also shows the project's most recent plan in
  progress ([the plan list](features-work-resume.md#the-plan-list-and-what-needs-you)).
- The **Inbox** lists what **needs you** in every chat (approvals can be
  answered there; with work plans, one item per plan waiting on you),
  chats that **finished while you were away**, and every **notice** in
  full while the app runs; its badge counts all three.
- A fresh install (no projects, no chats) first meets the [pet](features-desktop-pet.md)
  (its name and look), then sets up a model, a project and its trust, the
  permission mode and how lively the pet is. The pet stays above the setup
  card and reacts to each step.

**For developers**
- `stageFor()` in [`lib/domain/stage.ts`](../../crates/z-engine-gui/ui/src/lib/domain/stage.ts)
  picks home, chat or inbox for [`chrome/MainStage.svelte`](../../crates/z-engine-gui/ui/src/components/chrome/MainStage.svelte);
  [`components/home/`](../../crates/z-engine-gui/ui/src/components/home/),
  [`inbox/`](../../crates/z-engine-gui/ui/src/components/inbox/) (over
  `inboxSnapshot()`; `pushToast` records the last 200 notices) and
  [`onboarding/`](../../crates/z-engine-gui/ui/src/components/onboarding/)
  (when `needsOnboarding()`; the pet's pose per step is `onboardingPose()`
  in `lib/domain/onboarding.ts`) draw the rest.

## The transcript and tool cards

**In plain words.** The transcript is the chat history drawn as cards.
Routine tool work folds into one line, like a receipt you can unfold for
the details; failures stay in view.

**How it works**
- A turn shows your message, the reply (Markdown, with a collapsible
  thinking section), its tool calls and a one-line receipt. Two or more
  tool calls in a row fold into one line ("Read 2 files · ran 1 command")
  with the step in progress and how many failed or were denied; `Agent`,
  `AskUserQuestion` and `ExitPlanMode` stand alone. A failure shows its
  last three lines, a running command three live lines. Once a turn with
  two or more calls is done, all its work folds into one line ahead of the
  answer ("Worked for 1m 12s · read 6 files · ran 2 commands"); failures,
  helpers, questions and plans stay in view.
- A card follows `toolStarted`, `toolProgress` and `toolFinished` (ok,
  error, denied or cancelled) and depends on the tool's *family*: a diff
  for edits, command and output for Bash (both open by themselves only on
  failure), one line for Agent, own cards for TodoWrite and the web tools,
  a generic card for the rest, MCP tools included.
- The receipt follows `ui.task_report_view`: **quiet** (default) shows only
  a verdict that says something; **compact** adds the changed files (each
  opens the Changes tab there), duration and cost; **detailed** adds
  tokens, Not applicable and the checks. A turn that did not complete
  always says so. Beside it, the turn's actions: copy the answer, open the
  changes, rewind to before the prompt.
- A long chat draws only a window of turns: the newest 30, twenty more as
  you scroll up, and the newest six in full while the others fill in as
  they near the screen. A rail of dots marks your prompts (at most 40) and
  jumps to one. While a summary compaction runs (`compactionStarted`), a
  "Compacting context…" row ends the transcript; the marker then becomes a
  "Context compacted · 180k → 24k" divider with the summary.
- [Experimental decision features](features-decision-uses.md) add a few
  surfaces: a **Routed** chip (`routeChosen`), a task-view divider with
  **Include full history** (`taskViewApplied`, kept on reopen),
  **Claimed, not checked** on the receipt, and suggestion cards at the end
  of the transcript (`suggested`), which hide once the next turn starts.

**For developers**
- [`components/chat/`](../../crates/z-engine-gui/ui/src/components/chat/):
  `TurnView` with `WorkSummary`, `TurnBlocks`, `TurnActions` and
  `TurnFooter`; folds and the work line from `lib/domain/timeline/groups.ts`
  (`workSection()`, `workLine()`), receipts from `lib/domain/receipt.ts`,
  the turn window from `lib/domain/timeline/window.ts`, the rail in
  `ChatTimeline`; placeholders and code colors wait for `whenVisible()`;
  [`chat/tools/ToolCall.svelte`](../../crates/z-engine-gui/ui/src/components/chat/tools/ToolCall.svelte)
  picks the card from the family in [`lib/domain/tools/toolMeta.ts`](../../crates/z-engine-gui/ui/src/lib/domain/tools/toolMeta.ts)
  (a new card: a `FAMILIES` entry, a component, a branch in `ToolCall`).
- Decision surfaces: `chat/RouteChip.svelte` and `chat/TaskViewDivider.svelte`
  (through `LocalCards`), the claim in `TurnFooter` (`claimNote()` in
  `lib/domain/verification.ts`), and `planning/Suggestions.svelte` with
  `PlanSuggestion`, `RuleSuggestion` and `ReviewSuggestion` (state in
  `lib/domain/sessionView/suggestions.ts`).

## Approval, question and plan cards

**In plain words.** When the agent needs your say (permission, an answer, or
approval of a plan), it pauses and asks where you type, like a builder
who knocks before taking down a wall.

**How it works**
1. The engine's *gate* decides a tool call must ask (see
   [core features](features-core.md)) and emits `approvalRequested` with a
   title and a preview, such as the diff of an edit.
2. The card takes the composer's text box's place (the draft is kept) and
   asks a question ("Allow Bash to run cargo test?") with the reason and a
   preview folded after six lines: **Allow once**, **Always allow…** (**In
   this chat**, a session rule, or **In this project**, also saved to
   `.z-engine/settings.local.toml`), or **Deny…** with feedback for the
   model; y, s, p and n answer a focused card (`resolveApproval`).
3. `AskUserQuestion` shows a question card there too (`answerQuestion`);
   the oldest approval comes first, then the oldest question, and "N more
   waiting after this" counts the rest. `ExitPlanMode` puts a "Plan ready ·
   Review" row in the transcript and opens the side panel's Plan tab
   (`resolvePlan`); see
   [plan mode, questions and todos](features-interaction.md#plan-mode-structured-questions-and-todos).
   An untrusted project that sets anything trust withholds gets a trust
   banner in the transcript (`trustRequired`, answered with `trustWorkspace`).
4. While a card waits you can still change mode or cancel; while a plan
   waits you can also steer. Cards from a subagent carry that agent's label.

**For developers**
- [`components/planning/PendingInteractions.svelte`](../../crates/z-engine-gui/ui/src/components/planning/PendingInteractions.svelte),
  inside `chat/Composer.svelte`, renders `chat/ApprovalCard` (worded by
  `lib/domain/approvals.ts`) or `planning/QuestionCard`; plans are
  `planning/PlanReady` and `planning/PlanView` (`panelPlan()` in
  `lib/domain/plans.ts`). Those, and `chat/TrustBanner`, carry
  `data-pending-card`. Types: `ApprovalRequest`, `ApprovalDecision`,
  `PlanDecision`, `Question`, `QuestionAnswer`; the engine's
  [`broker/`](../../crates/z-engine-engine/src/broker/) holds pending
  requests so the session actor keeps accepting commands.

## The island and the pet

**In plain words.** The middle of the title bar is a small pill, the
*island*: the [pet](features-desktop-pet.md) and one line about what the agent is doing,
like a colleague you can see across the desk. A ring beside it shows how
full the context is; an amber count, how many other chats need you.

**How it works**
- The line shows one state, most urgent first: this chat needs you, a
  passing notice, a provider retry, work in progress (the step in plain
  words), a turn that just ended, a recap of turns that ended while you
  were away, or idle (the chat's title), with at most one number.
- It takes one of four shapes: at rest (the pet and the chat's title),
  live (a step, retry, notice or result with its clock), alert (this chat
  needs you, with one **Approve**, **Answer** or **Review** button that
  goes to the waiting card) and expanded. Clicking it grows it into a
  card: the full message and its actions, the plan, the latest steps, rows
  for helpers, context and waiting chats that open what they sum up, cost,
  and recent warnings. A long, failing or actionable notice opens it by
  itself; a click elsewhere or Esc closes it, before Esc reaches the side
  panel.
- The ring shows a percentage from 65% (amber; red from 85%); it and
  `/context` open the context card: use by prompt layer, **Compact now** and
  **Inspect prompt**. The amber count opens the waiting chat, or the Inbox.
- The pet's pose follows the same state: its eyes scan while the agent
  reads, it bobs while commands run, looks at you in amber when it needs
  you, cheers when a turn is verified and droops when one fails. Running
  agents orbit it; a ring shows the `TodoWrite` plan's progress.
- At the default **Lively** level (`ui.companion`) it also reacts to you
  (typing, scrolling back, quiet minutes, coming back) and may roam, while
  the island shows a small [portrait](features-desktop-pet.md#the-islands-portrait)
  of it looking its way; **Calm** reacts only to the agent and stays in
  the island; **Off** shows a dot.

**For developers**
- [`chrome/TitleStatus.svelte`](../../crates/z-engine-gui/ui/src/components/chrome/TitleStatus.svelte)
  holds `WaitingBubble`, `Island` (`IslandCapsule` and `IslandCard` in one
  surface) and `ContextBubble` (with `ContextCard`); `liveStatus()` in
  [`lib/domain/liveStatus.ts`](../../crates/z-engine-gui/ui/src/lib/domain/liveStatus.ts)
  picks the state and `lib/domain/island.ts` shapes the pill
  (`islandMode()`, `islandAction()`, `islandLine()`).
- `createLive()` in [`lib/stores/live.svelte.ts`](../../crates/z-engine-gui/ui/src/lib/stores/live.svelte.ts)
  computes the status, the pet's pose (`petPose()` in `lib/domain/pet/pose.ts`)
  and the clock once, for the title bar and the roaming pet;
  `pet/IslandPet.svelte` draws the pet in the island's slot (its
  `IslandPortrait` while the pet roams), and
  `lib/stores/island.svelte.ts` holds whether the island card and the
  context card are open.
- The card calls `context_breakdown` ([`Engine::context_breakdown`](../../crates/z-engine-engine/src/engine/queries/context.rs)),
  the estimate `/context` prints, which also arrives as `contextReport`.

## The side panel

**In plain words.** Beside the chat sits one panel with four tabs, like the
drawers of a desk: what changed, the plan, the helpers out on errands, and
exactly what was last sent to the model.

**How it works**
- The panel toggle in the title bar shows or hides it; each tab also opens
  from where its fact is summed up (the Changes button or ⌘D / Ctrl+D, a
  receipt's file, the island card's rows, the context card, `/agents`,
  `/jobs`, the palette). Its left edge resizes it (340 to 1100 px, always
  leaving the chat 400 px; the width is kept on this machine), and it can
  use the whole stage instead. A plan that starts waiting opens the Plan
  tab; a helper that starts marks Agents with a dot. Esc steps back: out
  of an agent's transcript, out of the whole stage, then closed.
- **Changes**: **This chat** compares the project with the chat's first
  *checkpoint*, a copy of your files kept in a hidden git repository,
  without taking a new one; **Uncommitted** compares the working tree with
  your last commit (`HEAD`), with line counts. Both need git. Unified or
  Split, with syntax colors and long unchanged stretches folded; a file
  can be opened in its default app, shown in its folder, or copied; `[`
  and `]` step through files. In a narrow panel the file list sits above
  the diff.
- **Plan**: the plan waiting for review (edit, approve, keep planning), else
  the chat's last plan, with the todo checklist. With experimental
  [work plans](features-work-plans.md) on, the attached plan instead: a
  header (status, goal, progress, revision, the coverage line that opens
  the audit's findings), a **Needs you** strip, **Now** lanes while it
  builds, **Tasks** or **Document**, a task's detail with its result, the
  notebook, and **Build** or the change set to accept. With
  [parallel tasks](features-work-parallel.md) on, rows say whether they
  can run beside others. A paused, done or abandoned plan has a banner
  (**Resume**, **Reopen**); held and interrupted tasks say so; with no
  plan, **Continue a plan** lists the project's others
  ([pausing and resuming](features-work-resume.md)).
- **Agents**: subagents (`agentStarted`, then `agentUpdated`) in the order
  they need you: **Ready to apply** (a worktree agent's branch and
  diffstat, with **Apply** and **Discard**), **Working** with a live line
  of what each does, then **Finished** and usage, folded; opening one loads
  its transcript (`agent_transcript`). A helper sent for a plan task names
  it, and the name opens the task in the Plan tab; an interrupted one says
  "Interrupted · kept for task …" and offers only **Discard**. **Jobs** lists background shells and
  agents (`jobUpdated`); a running job shows its last six lines and
  **Stop**.
- **Context**, the prompt inspector: it fetches the last request sent to the
  provider (`inspect_request`) and lays it out as a ring chart of the
  context window with one arc per kind of part (instructions, project,
  conversation, tools), whose legend filters an outline to search, and
  **Insights** (the largest part, what repeats across turns); **Copy all**
  copies it all. With the whole stage, a reader shows one part, rendered
  or raw; picking a part in the docked panel expands it. Below Insights,
  **Decisions** appears while an
  [experimental decision feature](features-experimental-and-decisions.md)
  runs or has recorded decisions: what runs and in which mode, who answers,
  speed, tokens saved, and the newest 50 decisions. It refreshes after each
  turn and when opened. With a work plan attached, **Task packet** shows
  what the agent was given about its task.

**For developers**
- [`components/sidepanel/`](../../crates/z-engine-gui/ui/src/components/sidepanel/)
  (`SidePanel`, `SidePanelTabs`); state `ui.panel` and `openPanel()` in
  `lib/stores/ui.svelte.ts`; tabs, widths and nudges in
  [`lib/domain/sidePanel.ts`](../../crates/z-engine-gui/ui/src/lib/domain/sidePanel.ts)
  (`panelTabs()`, `clampPanelWidth()`, `panelNudge()`), followed by
  `followPanelNudges()` in `lib/stores/panelNudges.svelte.ts`.
- Changes: [`overlays/DiffPanel.svelte`](../../crates/z-engine-gui/ui/src/components/overlays/DiffPanel.svelte)
  and its `Diff*` parts, rows from `lib/domain/diffRows.ts`. This chat:
  `session_changed_files`, `session_diff_for_file` ([`engine/queries/changes.rs`](../../crates/z-engine-engine/src/engine/queries/changes.rs)).
  Uncommitted: `list_changed_files`, `diff_for_file` ([`engine/queries/git.rs`](../../crates/z-engine-engine/src/engine/queries/git.rs),
  which also creates the worktree for **New chat in a worktree…**); they
  take a project root, else the active project, and refuse other folders.
  `open_path` and `reveal_path` ([`commands/workspace.rs`](../../crates/z-engine-gui/src-tauri/src/commands/workspace.rs))
  use the opener plugin once `guard::project_path` finds the path inside a
  known project, symlinks resolved.
- Plan: `planning/PlanView.svelte` over `panelPlan()` in `lib/domain/plans.ts`;
  with a work plan, `planPanel()` in `lib/domain/work/panel.ts` and the
  `Plan*` and `Task*` parts in `planning/`, over `SessionView.work`.
- Agents: [`components/agents/`](../../crates/z-engine-gui/ui/src/components/agents/)
  (`WorkPanel`, `ApplyCard`, `AgentRow`, `JobRow`, `AgentTranscript`;
  `AgentSummary` is the one helper layout, with a `PetSprite`, for both
  its rows and the transcript's Agent card), `agentSections` and
  `agentActivity` in `lib/domain/agentTree.ts`; commands
  `applyAgentChanges`, `discardAgentChanges`, `killJob`. How agents run:
  [agents and context](features-agents-and-context.md).
- Context: [`overlays/PromptInspector.svelte`](../../crates/z-engine-gui/ui/src/components/overlays/PromptInspector.svelte)
  with its `Inspector*` parts (the ring is `InspectorMap`); parsing in
  `lib/domain/requestInspect.ts`, the outline in `lib/domain/inspectOutline.ts`;
  data from `Engine::last_request`. Decisions: `overlays/InspectorDecisions.svelte`
  over `session_decisions` (`Engine::session_decisions`), worded by
  `lib/domain/decisionTrace.ts`. Task packet: `overlays/InspectorPacket.svelte`
  over `work_packet`.

## Settings and scopes

**In plain words.** Settings are stacked like transparent sheets: defaults,
then yours, then the project's, then your private project tweaks. The
settings screen picks the sheet you write on and shows where each value
comes from.

**How it works**
- Pages (⌘, or Ctrl+,) are grouped: **General** (Models, Providers,
  Appearance, Pet), **Agent** (Permissions, Memory, Verification, Agents &
  Commands), **Integrations** (MCP, Hooks) and **System** (Advanced,
  Experimental, About & Updates). Settings covers the window as a sheet
  that settles into place and reopens where you left it; search finds
  single settings by everyday words and scrolls to them.
- **Experimental** lists features this version includes that are still
  experimental (the 21 decision features, each with Off, Shadow and On,
  and work plans, with Off and On) and an "Experimental" tag. While a decision feature is
  Shadow or On, the **Decision model** card below sets up the model
  (runtime **Sidecar** or **In the app**, sidecar or endpoint, timeout,
  threshold, recording) and offers **Test connection** for the project
  open in Settings. With **In the app**, its **Native model** row shows
  whether the pinned files are downloaded, with **Download**, **Cancel**,
  **Resume** and **Remove**, polling progress twice a second.
- A layered page names the file it saves to (**Saving to**): **User** (every
  project), **This project** (`.z-engine/settings.toml`, shared) or
  **Personal (local)** (`.z-engine/settings.local.toml`, kept out of git).
  A badge names where a value came from; defaults have none. Saving writes
  that one file and reloads every open session for a user change, else
  that project's sessions.
- Agents, commands, skills, rules and `AGENTS.md` are edited as files, after
  a check that the path is one the app knows.

**For developers**
- [`components/settings/`](../../crates/z-engine-gui/ui/src/components/settings/):
  `SettingsPage.svelte` switches on the page ids of `SettingsNav.svelte`,
  grouped and searched through `lib/domain/settings/searchIndex.ts`; one
  `*Tab.svelte` per page; form logic in `lib/domain/settings/`. The page
  sits in `chrome/FullPage.svelte` (the `sheet` transition of
  `lib/ui/motion.ts`, a fade under Reduce Motion); `openSettings()` in
  `lib/stores/app-actions.ts` opens it at a page and a setting.
- Shell: [`commands/settings.rs`](../../crates/z-engine-gui/src-tauri/src/commands/settings.rs)
  (`set_setting`, `add_permission_rule`, `set_hooks`, `feature_catalog`,
  `test_decision_model`, ...), `layers.rs`, `commands/access.rs` (keys,
  trust), `commands/extensions.rs` and `guard.rs`.
- Experimental: `ExperimentalTab.svelte`, `DecisionModelCard.svelte` and
  `DecisionNativeModel.svelte`; the catalog in
  `lib/stores/features.svelte.ts`, listing and modes in
  `lib/domain/settings/features.ts` (also the search entries), test
  wording in `lib/domain/settings/decisionModel.ts`, the native model row's
  in `nativeModel.ts`; the shell
  commands `decision_model_status`, `download_decision_model`,
  `cancel_decision_model_download` and `remove_decision_model`.

## The composer, palette and shortcuts

**In plain words.** The composer is the message box; the command palette is
a search box for the whole app: press ⌘K, type a few letters, and jump to a
chat, a project, a setting or an action.

**How it works**
- The composer sends `submit`. While a turn runs, a new message is queued as
  steering (pills you can edit, sent as `editQueue`); ⌘Enter / Ctrl+Enter
  sends `interrupt`, and Esc or the stop button `cancel` (with an empty
  draft, Send itself turns into Stop). Its **+** menu
  attaches images and inserts `@` (mentions), `/` (commands), `#` (a memory
  note) and `!` (your own `shell` command, output in a drawer above it),
  and turns plan mode on or off (`setPlanning`). While plan mode is on, a
  **Plan** chip with an **×** sits beside the permission chip.
  A waiting approval or question takes the draft's place until answered.
- The palette (⌘K / Ctrl+K, or **Search** in the sidebar) lists actions,
  places, recent chats, this chat's tools, the pet's actions, modes and
  projects; typing also finds single settings.
- Global shortcuts (⌘ on macOS, Ctrl elsewhere): K palette, N new chat,
  B sidebar, D the Changes tab, comma settings. In the composer, Shift+Enter
  adds a line, Shift+Tab turns plan mode on or off, ↑ and ↓ walk your
  history. Outside it, Esc closes the island card first, then steps the
  side panel back; on Settings it goes back.

**For developers**
- [`chat/Composer.svelte`](../../crates/z-engine-gui/ui/src/components/chat/Composer.svelte)
  and its `Composer*` parts; keys in [`lib/domain/composerKeys.ts`](../../crates/z-engine-gui/ui/src/lib/domain/composerKeys.ts);
  app-only slash commands in `lib/stores/uiCommands.ts`.
- [`overlays/CommandPalette.svelte`](../../crates/z-engine-gui/ui/src/components/overlays/CommandPalette.svelte)
  with `lib/paletteActions.ts` and `lib/domain/palette.ts`; global keys in
  `lib/stores/shortcuts.ts`.

See also: [The desktop app](features-desktop-app.md) · [The desktop pet](features-desktop-pet.md) ·
[Experimental features and the decision layer](features-experimental-and-decisions.md) · [How HyperForm works](README.md) ·
[Everyday use](../user-guide/02-everyday-use.md) · [Agents](../user-guide/04-agents.md) ·
[Settings reference](../user-guide/12-settings-reference.md) · [The crates](crates.md)
