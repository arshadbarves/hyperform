# Changelog

All notable changes to the **HyperForm AI** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-06

### Added
- **HyperForm AI Initial Release**: Local-first, intelligent AI development environment (*"Think Beyond. Build Beyond."*).
  - Autonomous AI agents paired with developers for understanding, planning, executing, and evolving software.
  - Work plans with task trees, acceptance criteria verification, and parallel helper dispatch.
  - Granular permissions engine, sandboxed shell execution, and shadow git checkpoints.
  - Native multi-provider model support (Anthropic, OpenRouter, OpenAI, GitHub Copilot).
  - Intelligent context assembly, repo map, semantic compaction, and decision layer.
  - Cross-platform desktop interface built with Tauri and Svelte 5 with interactive 3D companion pet.
  - Support for Windows (ARM64 and x64), macOS (Apple Silicon and Intel), and Linux.

## [2.6.0] - 2026-10-06

### Added
- **Work plans (experimental)**: turn on `work_plans` in **Settings →
  Experimental** (Off or On; it has no Shadow mode) and a plan made in plan
  mode becomes a saved list of tasks, each with acceptance criteria. Only
  you can approve the plan, confirm or waive a criterion, or change the
  goal; the agent can't mark a task done while a criterion has no evidence
  from a passing check. The agent works through the new `TaskCreate`,
  `TaskUpdate`, `TaskGet`, `TaskList` and `PlanRecord` tools, which are
  offered only while the feature is on. The **Plan** tab shows the tasks,
  their criteria and a notebook of decisions, discoveries, experiments and
  changes; **Build in a new chat** hands the plan to a fresh chat. Each turn
  the agent gets a short *task packet* for its current task (see it in the
  **Context** tab), the plan survives compaction, `/replan` asks the agent
  to revise the plan, and when the workspace no longer matches the plan
  (a deleted file, stale evidence, overlapping or duplicate tasks) the plan
  goes back to the agent to revise. New `[work]` settings (`packet_tokens`, `sensitive_paths`,
  `max_attempts`); plan logs are saved under `plans/` in the data folder.
  Once the agent reads a web page in a turn (`WebFetch`, `WebSearch`, or a
  helper that did), what it adds to the plan for the rest of that turn is
  marked **from web** and waits for you: new tasks are **Proposed** until
  you **Accept** or **Dismiss** them, notes and a helper's reported
  findings stay unconfirmed, and that turn can't change the tasks already
  in the plan. Resuming a helper that read the web counts as reading it
  again.
- **Helpers on plan tasks (with work plans)**: the main agent can send a
  helper for one ready task (`Agent` with `task_id`). The helper owns that
  task, asks before writing outside the task's files, and reports a
  result that the task's new **Result** section shows next to what Z
  Engine observed (files changed, checks run). A helper that ends without
  a report leaves its task blocked for you to retry. Your **Retry** gives a
  task a fresh set of attempts. The Agents tab and Agent cards name the
  task.
- **Parallel tasks (experimental)**: turn on `work_parallel` (Off or On;
  needs `work_plans`) and, once you allow it per plan (**Build with
  helpers in parallel**, or **Allow helpers in parallel** while building),
  independent tasks run at the same time in helpers, each in its own
  worktree. Tasks that share files or symbols, depend on each other, name
  no files or touch a serial path (lockfiles, manifests, generated output)
  run alone; task rows say why. Changes come back one at a time: you apply
  each one (or set `merge = "auto"`), then the task's checks run in your
  project before it is done; if a check couldn't run, **Run checks again**
  retries them. A conflict leaves your files untouched and
  offers **Retry serially**. New `[work]` settings `max_parallel` (3),
  `merge` (`"ask"`) and `serial_paths`; an untrusted project can't turn on
  automatic merges or remove serial paths.
- **Plan audit (with work plans)**: every plan change is checked for
  requirements without tasks, tasks without criteria, criteria nothing can
  check, risky paths without a test, risks without a mitigation, and
  overlapping or duplicate tasks; the new read-only `audit` agent reviews
  deep plans (judged from the whole plan when planning ends) and runs on
  **Run audit**. The Plan tab's coverage line ("4 of
  5 requirements covered · evidence Fair · 2 findings") opens the
  findings, each with **Add to plan** or **Dismiss**, and a **Needs you**
  strip lists conflicts, changes to apply, stopped tasks, checks to run
  again, proposals and high-severity findings (medium and low ones stay in
  the findings list).
- **Pause, hold and stop a plan (with work plans)**: the plan menu has
  **Pause**, **Request replanning**, **Stop** and **Abandon the plan**
  (both confirmed), **Switch to another plan…** and **Detach from this
  chat**; a task's menu has **Hold** and **Release**. Pausing or holding
  lets running helpers finish their current step, then their tasks wait
  with their worktrees kept; **Stop** cancels every helper at once and
  pauses the plan. Closing a chat or detaching its plan stops its helpers
  the same gentle way. A plan whose last task is done becomes **Done** by
  itself, with a banner summing up what was verified; **Reopen** builds it
  again. An interrupted start doesn't count toward `work.max_attempts`.
- **Pick a plan back up (with work plans)**: when a chat opens with its
  plan (also after a restart) or you attach one, tasks whose helper is
  gone wait again with a note on where they stopped, files changed outside
  the plan are checked against it, the transcript shows a quiet **Plan
  resumed** row, and the agent's next turn starts from where the plan
  stands. It can continue an interrupted helper in its own worktree
  (`Agent` with `task_id` and `resume`). **Continue a plan** (the empty
  Plan tab, the composer's **+** menu, the palette, the plan menu) lists
  the project's plans with their progress and what needs you; one chat
  drives a plan at a time, so continuing a plan from another chat moves it
  here and that chat says "This plan moved to another chat.". The
  project home's **Continue** card shows the plan in progress, the
  Inbox has one item per plan that needs you, and the Agents tab marks an
  interrupted helper "Interrupted · kept for task …" with **Discard**
  only.

### Changed
- **A failure holds only what it affects (work plans)**: when a failure,
  a replaced task, overlapping tasks or a changed file sends a building
  plan to **Replanning**, only the tasks the problem names and the tasks
  that depend on them wait ("waits for the plan change"); the other ready
  tasks keep running. Accepting the changes, or the problem going away,
  releases them. Replanning you start (plan mode, **Request replanning**,
  `/replan` with your own request) still holds every task.
- **Plan mode is a switch, separate from the permission mode**: the
  permission modes are now Ask, Auto-accept edits and Bypass, and plan mode
  turns on or off on top of any of them. While it is on, anything that
  changes files or state is refused, including commands Z Engine can't
  prove read-only (such as `cargo test`); everything else follows the
  permission mode, so plan mode with Bypass researches without asking and
  still changes nothing. **Shift+Tab** now turns plan mode on or off
  instead of cycling permission modes; a **Plan** chip with an **×** sits
  beside the permission chip while it is on, and the composer's **+** menu
  and the palette offer **Plan first** / **Stop planning**. `/mode plan`
  turns it on or off.
- **One Build button for a plan**: **Build** turns plan mode off and keeps
  your permission mode, replacing **Approve & auto-accept edits** and
  **Approve & ask before edits**.
- New setting `permissions.planning` (**Settings → Permissions → Start new
  chats in plan mode**), which an untrusted project may also set since it
  only restricts. `permissions.mode = "plan"`, agent frontmatter
  `permissionMode: plan` and chats saved in plan mode keep working.
- Subagents inherit plan mode and can't turn it off; the read-only
  `explore`, `plan` and `review` keep refusing changes in Bypass. Hooks
  still receive `permission_mode: "plan"` while plan mode is on.

### Fixed
- The documentation website build finds the app icon from the README
  (`./docs/assets/z-engine.png`) instead of treating the path as a package.

## [2.5.0] - 2026-10-04

### Added
- The README, documentation home and docs website share the app icon
  (GitHub's repo About and link previews use it).
- **Free tag for OpenCode Zen models**: the model picker and the model
  suggestions in **Settings → Models** tag Zen models whose catalog input
  price is 0 as **Free**. Only OpenCode Zen gets the tag, since other
  providers (such as GitHub Copilot) list plan-included models at a price
  of 0.

### Changed
- **OpenCode Zen without a key lists only its free models** in the model
  picker and the Settings suggestions, since paid Zen models fail without
  a key. With a key stored or set in `OPENCODE_API_KEY` or
  `ZENGINE_API_KEY`, every Zen model is listed, free ones first.
- **Retired models are hidden** from the model picker and suggestions for
  every provider, when models.dev marks them `"status": "deprecated"`. The
  catalog still uses them for limits and prices, **Another model id…**
  still accepts any id, and a `models.json` entry can set `"status"` too.

### Fixed
- **Start free** in the setup guide, and **Connect** on OpenCode Zen, now
  default to `big-pickle` instead of `deepseek-v4-flash-free`, which
  models.dev marks retired.

## [2.4.0] - 2026-10-04

### Added
- **GitHub Copilot provider**: connect it in **Settings → Providers** or
  the setup guide by signing in with GitHub (GitHub.com or a GitHub
  Enterprise domain) instead of pasting a key, then chat with the models
  your Copilot plan includes. Each model is sent in the format Copilot
  serves it in (Anthropic Messages, OpenAI Responses or chat completions);
  tool rounds count as the agent's requests, not yours. The token is
  stored in `auth.json` under `github-copilot`, and **Sign out** removes
  it. Turn cost, context window and output limit come from the catalog's
  GitHub Copilot listing for each model.

### Changed
- **Cost and limits use the active provider's listing**: when the
  models.dev catalog lists the model under the provider you use (GitHub
  Copilot, OpenRouter, Anthropic, OpenAI, OpenCode Zen, Groq or LM Studio),
  its price, context window and output limit come from that listing
  instead of the first provider's for the same model id. For example,
  OpenCode Zen, Groq and LM Studio models now use those providers' own
  listings rather than OpenRouter's or the maker's. `[pricing]`,
  `model.context_window` and `model.max_output_tokens` still win; other
  providers are handled as before.

## [2.3.0] - 2026-10-01

### Added
- **Experimental features**: new features now start in **Settings →
  Experimental** (or `[experimental]` in a settings file), each Off,
  Shadow (runs and records what it would have done, without changing
  anything) or On, until they meet a measured bar and become standard.
  This version includes 21, all decision features, all Off by default.
- **The decision model**: a small local model (laya-serve or a
  Jev-compatible server) that the experimental decision features ask
  quick questions, set up in `[decisions]`: an address on this computer
  or a sidecar command Z Engine starts and stops, timeout, confidence
  threshold, per-question calibration, **Test connection**, and an opt-in
  dataset in the data folder; a **Decisions** section in the Context tab
  shows what each feature decided. Every unanswered, slow or unsure
  question keeps today's behavior, and a decision can only add an
  approval, a notice, a note or a card, never allow anything.
- **Native runtime (experimental)**: native in-process runtime for the
  decision model (`decisions.runtime = "native"`), with a pinned, verified
  model download in Settings (resumable, checked against SHA-256
  checksums, removable). It needs a build with the `onnx` feature; the
  release installers don't include it yet.
- **Smarter context (experimental)**: relevance-aware compaction,
  task-scoped history (earlier exchanges set aside at a new task, with a
  divider and **Include full history**), first-request context, file
  prefetch (`[decisions.prefetch]`), and relevant trimming of long command
  output and search results.
- **Per-task routing (experimental)**: the reasoning effort picked once
  per task while yours is on auto, optionally the fast model for simple
  tasks (`[decisions.routing] allow_model_switch`), shown as a **Routed**
  chip.
- **Safety reviews (experimental)**: risk review approvals and warnings on
  web and MCP results that try to instruct the agent; secret screening,
  which asks before likely credentials in tool output reach your provider;
  your own decision rules (`[[decisions.rules]]`); and a loop guard that
  reminds a stuck agent, then asks you.
- **Verification help (experimental)**: a completion check that marks
  **Claimed, not checked** when the final message claims success without
  a check, and check selection that skips checks the changes can't affect.
- **Suggestions and guidance (experimental)**: "Plan first?", "Remember
  this for next time?" and "Run a review?" cards; a correction signal;
  skill and agent hints; and a check that points the agent to your earlier
  answer instead of asking again.
- **Pet mood and inbox priority (experimental)**: the pet's mood after a
  turn follows how the turn went, and the Inbox sorts what needs you by
  urgency, with `Notification` hooks firing only for urgent or unrated
  items.

### Changed
- **The pet is now 3D**, drawn with WebGL, with every mood, prop,
  accessory, stage, move and trick it had. It turns its body to face where
  it goes (its spin trick is a full turn), has glossy eyes, and its face
  slides over its body as it looks around. It draws frames only while
  something plays, so an idle pet costs nothing. Where WebGL is missing, or
  after the graphics context is lost, the flat pet stands in; the boot
  splash and the helper sprites stay flat.

### Fixed
- Bypass mode now applies to subagents: agents that set their own
  `permissionMode` (the built-in `explore`, `plan` and `review` run in
  Plan) no longer ask or refuse while the chat is in Bypass. Deny rules
  still apply.
- The island no longer looks empty while the pet roams: instead of a faint
  dash, its slot shows a small live portrait of the pet's head that turns
  toward where the pet is, in a thin ring of the status color (breathing
  while the agent works, amber when something needs you). The island keeps
  its width; **Calm** and **Off** are unchanged.
- **Customize…** on the pet's card no longer raises an error as the card
  closes and **Settings → Pet** opens.

## [2.2.0] - 2026-09-30

### Added
- **The pet**: a small creature, **Zen** by default, replaces the
  title-bar companion orb. Its face and posture show what the agent is
  doing (working, needs you, done, failed) across 34 moods, it gets sleepy after quiet
  minutes and greets you when the app opens (it wakes up on the splash,
  then flies into the title bar). At the **Lively** level it roams while
  nothing needs you: above **What should we work on?** on the project
  home, onto the composer's top edge, the sidebar footer and the open side
  panel's tab bar, and into the empty Inbox; it watches the composer while
  you type, naps after 3 minutes, rides back into the island while the
  agent works and hops in when something needs you. It walks along edges
  and hops between spots on springs. Drag it anywhere (it swings from your
  grip), throw it (a hard throw leaves it dizzy), click to boop it,
  double-click or right-click for its card.
- **Pet growth**, saved in `<data dir>/pet.json`: XP for completed turns
  (10), verified turns (15 more), applied helper worktrees (20) and the
  first of these each day (5 more, with a day streak). Levels change its
  shape (Seed, Sprout, Bloom, Star), unlock things to wear (sprout, scarf,
  headphones, star) and idle tricks (stretch, hop, spin, sparkle). At
  Lively a new level plays a short confetti celebration once nothing needs
  you.
- **Pet card**: level ring, name, level and stage, XP to the next level,
  streak, turns, verified and applied counts, what it wears, its tricks,
  and **Customize…**, **Stop roaming** / **Let it roam** and **Call back**.
- **Settings → Pet**: a preview with **Show its card**, **Name**, **Look**
  (Pearl, Mint, Sky, Lilac, Peach, Graphite), **Liveliness** and **Let it
  roam**; Settings search finds each one. **About & Updates** lists
  `pet.json`.
- New settings `[ui.pet]`: `name` (default `"Zen"`, up to 24 characters),
  `look` (default `"pearl"`) and `roam` (default `true`).
- First-run setup starts by meeting the pet (**Hi, I'm Zen**, a name and a
  look), keeps it above the setup card on every step, and asks in the work
  style step whether it roams.
- Command palette **Pet** actions: **Rename Zen**, **Change Zen's look**,
  **Show Zen's card**, **Stop Zen roaming** / **Let Zen roam** and **Call
  Zen back** (with your pet's name).
- **Side panel**: one panel beside the chat with **Changes**, **Plan**,
  **Agents** and **Context** tabs, shown or hidden from the title bar.
  Drag its edge to resize it (the width is kept), let it use the whole
  stage, and press Esc to step back; ← and → switch tabs. A plan waiting
  for review opens the Plan tab, and a new helper marks Agents with a dot.
- **Plan tab**: the plan waiting for review, with **Edit** / **Preview**,
  **Approve & auto-accept edits**, **Approve & ask before edits** and
  **Keep planning**, else the chat's last plan; the todo checklist shows
  **N of M done**.
- **Turn actions** under each answer: copy the answer, **Open changes** and
  **Rewind** (to before this prompt: code and conversation, conversation
  only, or code only).
- A finished turn's work folds into one line such as **Worked for 1m 12s ·
  read 6 files · ran 2 commands**, with failed and denied counts.
- **Compacting context…** shows while a summary is being written, before
  the **Context compacted** divider; new protocol event
  `compactionStarted { trigger }`.
- Double-clicking an empty part of the title bar, the sidebar's head, the
  side panel's head or the Settings bar maximizes or restores the window.

### Changed
- **Dark frosted redesign**: the app is always dark, over the dark HUD
  vibrancy on macOS (active even when the window is in the background) and
  dark Mica on Windows 11, solid on Windows 10 and Linux. Four layers (the
  window, the content sheet, glass for the sidebar card, side panel,
  composer and island, stronger glass for popovers, menus and dialogs) use
  one glass recipe with no gradients and a blue accent. Motion runs on
  springs and moves only position and opacity: the home, a chat and the
  Inbox fade up as you switch, and panels slide rather than resize. The
  sidebar is a floating card that slides away, with the macOS window
  buttons inside its head; its footer no longer has the model chip (pick
  the model in the composer, set the default in **Settings → Providers**).
- **Approvals and questions wait in the composer**, in place of the text
  box: the oldest approval first (it takes the focus unless you are typing
  elsewhere), else the oldest question, with **N more waiting after
  this**; your draft comes back once they are answered. A plan shows as a
  **Plan ready · Review** row in the transcript and opens the Plan tab.
- **Island**: it rests, shows live steps and notices, and while the chat
  needs you offers one **Approve**, **Answer** or **Review** button that
  goes to the waiting card. Clicking the island itself, amber or not,
  grows it into a card (the notice
  and its actions, the plan, the latest steps, rows for helpers, context
  and waiting chats, cost, recent warnings) instead of a sheet.
- The Changes and Agents panels and the full-window prompt inspector are
  the side panel's **Changes**, **Agents** and **Context** tabs. The
  Context tab keeps the ring map, outline and **Insights** docked and adds
  the reader when it uses the whole stage.
- **Rewind** moved from your message to the turn's actions, and copying the
  answer moved there too; your message keeps **Copy** on hover.
- Long chats draw the newest 30 turns and 20 more each time you scroll near
  the top (a click on the prompt rail draws the turns back to that prompt);
  older turns and code colors fill in only as they near the screen, so a
  1,000-turn chat scrolls smoothly and an idle window does almost no work.
- Dialogs share one layout (an optional icon, the title and a line under
  it, then the body, the actions last), and **Settings → Providers** groups
  the endpoint and model new chats start with under **In use**.
- `ui.companion` (**Lively**, **Calm**, **Off**) moved from **Settings →
  Appearance** to **Settings → Pet** as **Liveliness**; **Calm** now keeps
  the pet in the title bar, and **Off** still shows a small dot.
- Send and Stop in the composer are one button: while the agent works and
  the draft is empty, Send turns into Stop.
- Helper calls use one summary line with a tiny sprite of the pet, in the
  transcript's Agent card and in the Agents tab's rows.
- The prompt inspector's map is a ring chart of the context window; its
  legend filters the outline.
- The sidebar's selection highlight slides between rows; Settings opens as
  a sheet over the window; the project home without a project centres its
  empty state.
- The splash shows the pet instead of the Z mark and the name: it drops
  in, wakes up, breathes and blinks while the app loads, then flies into
  its place in the title bar (or above the setup card on a fresh install).
  With Reduce Motion it simply fades.
- The app icon, the favicon and the logo in **Settings → About & Updates**
  show the pearl pet glowing on a midnight squircle instead of the Z mark.

### Removed
- The title-bar companion orb and its **Companion** group in **Settings →
  Appearance** (the pet and **Settings → Pet** replace them).

### Fixed
- Glass stays blurred after the first-launch entrance animations.
- The idle pet no longer redraws every frame, for lower CPU and battery use
  while nothing happens.
- The Changes tab stacks the file list above the diff when the side panel
  is narrow.
- Esc closes the island's card before the side panel.
- A compaction divider at the end of a turn sits after the turn's actions.

## [2.1.0] - 2026-09-25

### Added
- **User guide** (`docs/user-guide/`): complete documentation for using the
  app, from getting started to the settings reference and troubleshooting.
- **How Z Engine works** (`docs/how-it-works/`): every feature and crate
  explained in plain words first, then the mechanism, then developer detail,
  with a glossary.
- **Documentation contract** (`docs/AGENTS.md`), a `docs-maintainer`
  subagent, and a CI link check, so feature changes update the docs in the
  same change.
- **Documentation website** at
  [arshadbarves.github.io/z-engine](https://arshadbarves.github.io/z-engine/):
  the user guide, how it works and the contributor docs, with search,
  published from `docs/` whenever the docs change on `release`.
- **Title bar island**: the companion orb and one line in the middle of the
  title bar say what the agent is doing in plain words, with one number
  (elapsed time, the turn's duration, or a retry countdown). The orb shows
  the same state as body language, orbited by running helpers, and reacts
  to you at the default **Lively** level. Click the island for the live
  steps, plan, helpers, cost and recent warnings; long notices, errors and
  notices with actions open it by themselves, and when the chat needs you,
  clicking it scrolls to the card. Beside it, an amber count of other chats
  that need you and a ring showing how full the context is (click it, or
  type `/context`, for the context card with **Compact now** and **Inspect
  prompt**). New setting `ui.companion` (`lively`, `calm`, `off`) under
  Settings → Appearance.
- **First-run setup** on a fresh install: connect a model (free OpenCode Zen
  models with no key, your own API key, or Ollama and LM Studio), add and
  trust a project, and choose the permission mode and companion. An
  animated splash shows while the app loads.
- **Project home**: the project and branch, a centred composer, starter
  prompts that fit the project, and cards for recent chats, uncommitted
  changes and project setup.
- **Inbox**: approvals (answerable in place), questions, plans and trust
  requests from every chat, chats that finished in the background, and
  every notice in full, kept for the app session. The sidebar badge counts
  what is waiting.
- Real window translucency: macOS vibrancy and Windows 11 Mica show through
  the window chrome; other systems get solid surfaces.

### Changed
- New visual system for the desktop app: design tokens with system colors
  and SF Pro, glass only for chrome that floats above content, a solid
  content sheet, spring and blur-in motion, and Reduce Motion and Reduce
  Transparency support.
- **Sidebar**: **New chat**, **Search** (the full command palette), **Home**
  and **Inbox** at the top; **Projects** (formerly Workspaces) with their
  branch, uncommitted-change count and a right-click menu, and one status
  dot and age per chat; the model in use (amber **Connect a model** when a
  key is missing), an **Update** button and Settings at the bottom. The
  title bar keeps only the changes button, with the number of files this
  chat changed.
- **Transcript and composer**: runs of tool calls fold into one line, small
  edits and successful commands stay folded, and subagents take one line.
  The turn receipt follows **Settings → Appearance → Task report detail**.
  Approval cards ask a question ("Allow Bash to run cargo test?") with
  **Allow once**, **Always allow…** (**In this chat** or **In this
  project**) and **Deny…**. The composer has a **+** menu, mode and model
  chips with reasoning effort inside the model picker, **Stop**, and an
  "Enter queues · ⌘Enter interrupts" hint; `!` output opens in a **Shell**
  drawer above it.
- **Changes panel** (formerly Review): **This chat** or **Uncommitted**,
  **Unified** or **Split**, a whole-window view, a compact file tree, file
  actions (open, reveal, copy the path or diff), syntax colors and folded
  unchanged lines.
- **Agents panel**: worktree changes **Ready to apply** come first, then
  running helpers with what each is doing now; finished helpers and usage
  fold away. Jobs show their live output and **Stop**. **New chat in a
  worktree** is a short dialog.
- **Prompt inspector** (formerly Context & Memory): a map of what fills the
  context window, an outline by kind with search, **Reader** and **Raw**
  views, and **Insights**.
- **Settings and palette**: Settings pages are grouped as General, Agent,
  Integrations and System; search finds single settings; a **Saving to**
  menu replaces the scope bar; long pages fold; source badges show only
  when a file sets a value; Settings reopens on the last page. The command
  palette is grouped, lists single settings as you type, and ranks results
  better.

### Removed
- The top-bar update chip, **Agents & jobs** button and Working/Review
  label, the working pill under the transcript, the todo strip above the
  composer, the live verification badge and the separate toast capsule.
  The title-bar island replaces them.
- Toasts that only confirmed something already visible (chat deleted,
  workspace added or removed, worktree created), the command palette's
  starter prompts (they remain on the home screen), and the home screen's
  logo, badge, tagline and shortcut rows.
- The historical v1 design documents, the old roadmap, `docs/deviations.md`
  and the old design specs under `docs/superpowers/`; git history keeps
  them.

### Fixed
- Deleting a chat or removing a project asks you to confirm first.
- The Changes panel's `[` and `]` keys (also `k` and `j`) step through the
  files; the diff panel's tooltips promised them, but nothing handled them.
- **Settings → Appearance → Task report detail** now changes what finished
  turns show; it used to be saved but ignored.
- The Hooks tab keeps the matcher of `SessionStart` and `PreCompact` hooks
  when it saves them, and offers a matcher field for both.
- OpenCode Zen free models no longer fail with `FreeTierError` ("can only
  be used from within OpenCode"): requests use the gateway's required
  client fingerprint (OpenCode user-agent, session id shape, and
  `shell`/`read` tool declarations).
- `verification.auto_checks` accepts a custom check by the id you gave it
  (`e2e`) as well as by its full id (`custom:e2e`).
- A built-in language server can be turned off with `enabled = false`
  alone; it no longer needs a `command`.
- The model catalog refreshes in the background once the cached copy is a
  day old, so newly released models appear without deleting the cache.
- The WebFetch setting (`web.fetch_extract`) is described as what it does:
  the fast model answers the agent's question about the page.
- The **Trust this workspace** setting and the chat's trust notices name
  everything an untrusted project is held back from; the list used to stop
  at hooks, MCP servers and checks.

### Security
- An untrusted workspace can no longer loosen your settings: its permission
  mode, allow rules, extra directories, shell, provider, web and
  language-server settings are ignored until you trust it (its deny and ask
  rules still apply). Its custom agents cannot run in a looser permission
  mode than their caller, and its custom commands' `allowed-tools` grant
  nothing.

---

## [2.0.0] - 2026-09-24

A from-scratch rewrite of the engine, tools, persistence and the desktop
runtime. See the [v2 engine architecture](docs/architecture/v2-engine.md).

### Added
- **Multi-agent orchestration**: built-in agents (`general`, `explore`,
  `plan`, `review`, `verify`) and custom agents from markdown files run in
  parallel, nested, in the background, or resumed, each with its own tools,
  model and permission mode, per-agent usage and cost, and an agent tree and
  jobs panel in the app.
- **Worktree isolation**: an agent can work in its own git worktree; its
  changes are applied with a three-way merge (conflicts reported) or
  discarded.
- **Claude Code-compatible tools**: `Read` (images, PDFs, notebooks),
  `Write`, `Edit`, `MultiEdit`, `NotebookEdit`, `Glob`, `Grep`, `Bash` with
  background jobs, `JobOutput`, `JobKill`, `WebFetch`, `WebSearch`,
  `TodoWrite`, `AskUserQuestion`, `ExitPlanMode`, `Skill`, `Agent`,
  `ApplyAgentChanges`, `Verify`, `LSP`, MCP resources and `LoadMcpTools`.
- **Planning and interaction**: todo strip, structured questions, and plan
  mode with an editable plan approval card.
- **Steering and interrupts**: messages sent while the agent works are
  injected at the next step; Cmd+Enter interrupts and redirects.
- **Hooks** for every agent event with a Claude Code-compatible JSON
  protocol (block, rewrite input, add context, force continuation).
- **Slash commands**: session commands, `/init`, `/review`,
  `/security-review`, `/commit`, custom markdown commands with arguments,
  file inclusion, inline shell output and turn-scoped tool grants, and MCP
  prompts.
- **Verification badges**: checks are discovered for most ecosystems, runs
  are recorded as evidence, and each turn is marked Verified, Unverified or
  Failed; optional `auto` and `strict` modes.
- **MCP over stdio and streamable HTTP**, deferred loading for large tool
  sets, and a multi-language **LSP** integration with post-edit diagnostics.
- **Native Anthropic provider** with prompt caching and extended thinking;
  cache breakpoints for OpenRouter; fallback models; a models.dev catalog
  with cache-aware cost.
- **Durable checkpoints** in a shadow git repository (shell changes
  included) and rewind of code, conversation, or both.
- **Optional sandbox** for shell commands (macOS seatbelt, Linux
  bubblewrap) with opt-in auto-approval.
- **Workspace trust** for project-defined hooks, MCP servers and checks.
- Hierarchical instructions (`AGENTS.md`, `CLAUDE.md`, nested files,
  glob-scoped rules), a multi-language repo map, and compaction that can
  summarize inside long agentic turns.
- Settings screens for models, providers, permissions, hooks, agents and
  commands, MCP, verification, memory and advanced options, per scope.

### Changed
- Settings live in `settings.toml` (user, project, project-local) with
  allow/ask/deny permission rules such as `Bash(npm test:*)`.
- Sessions are stored as one folder per chat with subagent transcripts and
  artifacts.

### Migration
- v1 `config.toml` files are never modified: the user file is imported once
  into `settings.toml`, project files are read in memory until the first
  change made in the app.
- v1 chats are imported on first open; the original files stay untouched.
- API keys in `auth.json` keep working. Keys for custom endpoints are now
  only sent to their own host and may need to be entered again.

### Removed
- The v1 crates (`z-engine-core`, `z-engine-provider`, `z-engine-runtime`,
  `z-engine-project`) and the task-report views they fed.

---

## [1.4.6] - 2026-09-15

### Added
- **Procedural Apple-Grade Brand Icon & Logo**:
  - Bespoke procedural generator (`scripts/generate_icons.py`) mathematically constructing Apple-grade continuous squircle, liquid titanium kinetic Z geometry, radiant solar combustion core, machined chamfer highlights, and multi-stage drop shadows.
  - Complete multi-platform desktop asset suite: macOS `icon.icns`, Windows `icon.ico`, multi-resolution PNGs (`icon.png`, `icon-512.png`, `icon-256.png`, `128x128.png`, `64x64.png`, `32x32.png`, Windows Store & Square logos), `icon.svg`, and web `favicon.svg`.
  - In-app `LogoMark.svelte` updated with kinetic Z geometry and lightweight gradients for crisp rendering across all UI surfaces (TopBar, Sidebar, HomeScreen, About modal).
- **Supervised Task Runtime & Verification Architecture**:
  - Bounded task supervision, durable verification gate, evidence collection, and task report projection.
  - Refined GUI components including TaskReportCard, CheckEvidenceDetails, ActivityTabs, AppearanceSettings, process disclosure, and settings cards.
  - Specialized bounded crates: `z-engine-context`, `z-engine-runtime`, and `z-engine-project`.

---

## [1.4.5] - 2026-09-07

### Fixed
- **Read-Only Filesystem on macOS App Launch**: Fixed `os error 30` when connecting providers or saving settings from the GUI by persisting general settings to the user's global configuration (`~/.config/z-engine/config.toml`), and gracefully falling back to saved workspaces or user home directory on launch instead of the root filesystem (`/`).
- **Workspace Project Context Synchronization**: Automatically sync `ctx.project_root` when a new workspace is added in the GUI.

---

## [1.4.4] - 2026-09-05

### Added
- **Minimal Premium Chat Redesign**: Collapsed noisy auxiliary tool activity into subtle, single-line process strips with in-place Apple segmented inspector (All, Files, Searches, Terminal, Reasoning).
- **Floating Scrubber Timeline**: Unobtrusive right-rail conversation scrubber with clean prompt tooltips, zero turn numbers, and razor-sharp hairline borders.
- **Apple Frosted User Bubbles**: High-contrast dark bubbles with smooth hover-reveal micro-actions (copy, edit/revert) and image attachment previews.
- **Floating Composer Dock**: Unified toolbar controls with circular send/stop button and modular architecture meeting the file budget.
- **Settings Redesign**: Clean Apple preferences layout with dedicated Providers directory, connection modals, and permission safety controls.
- **Prompt Inspector**: Consolidated context meter eliminating duplicate token counters.

---

## [1.4.1] - 2026-09-01

### Fixed
- **Auto-Updater Signature Verification**: Updated `pubkey` in `tauri.conf.json` to match the Minisign signing key used in GitHub Actions release workflow.
- **Unified Version Architecture**: Centralized version management via `scripts/bump-version.sh` and dynamic version extraction in packaging and UI.

### Added
- **Integrated Changelog Viewer**: Added dynamic changelog fetching and markdown viewer inside Settings > About with offline embedded fallback.

---

## [1.4.0] - 2026-08-31

### Added
- **Master-Detail Workbench Changes**: Replaced accordion cards with a sleek Master-Detail review layout featuring a file explorer sidebar on the left and full-height syntax-highlighted diff viewer on the right.
- **Virtualized Diff Chunking**: High-performance line chunking and file pagination capable of rendering repositories with 10,000+ changed files smoothly.
- **Dual Diff Scopes**: Switch between **Current Session** net modifications and **Git Working Tree** uncommitted changes.
- **Fluid Docking Animations**: Added `0.24s cubic-bezier` slide-in and slide-out transitions on opening and closing side panels.
- **Docked Worktree Panel**: Migrated Git Worktree creation from a floating modal to a docked side container.
- **TopBar Redesign**: Search trigger converted to a compact icon button with centered draggable window titlebar badge.
- **Sidebar Marquee & Fade Blur**: Added right-edge gradient fade mask and hover marquee effect for long workspace titles.

### Fixed
- **Typography Descender Clipping**: Corrected line-height and vertical baseline alignment to prevent descender clipping (`g`, `y`, `p`, `q`, `j`) in file lists.
- **Floating Terminal Overlay**: Decoupled terminal popup from composer flex flow and fixed autocomplete dropdown keyboard scrolling.

---

## [1.3.0] - 2026-08-27

### Added
- **Custom Luxury Provider Picker**: Custom dropdown selector in settings for OpenRouter, Anthropic, OpenAI, and DeepSeek.
- **Live Memory & Headroom Stats**: Integrated real-time cache analytics and token headroom metrics.
- **Apple Fluid Physics**: Spring physics on dialogs and overlays.

---

## [1.2.0] - 2026-08-20

### Added
- **Multi-Workspace Management**: Support for switching and managing multiple git repositories from the sidebar.
- **Session Checkpoint & Rewind**: Tree-based session checkpointing and step-by-step turn revert.
- **Model Catalog Integration**: Live OpenRouter and provider model catalog fetching with context window stats.

---

## [1.1.0] - 2026-08-15

### Added
- **Tauri Desktop GUI Frontend**: Svelte 5 + Bits UI high-performance desktop client.
- **Dual Frontend Architecture**: Terminal UI (Ratatui) and Desktop GUI sharing unified `z-engine-core`.
- **Streaming Tool Approvals**: Interactive permissions engine with per-command rule persistence.
