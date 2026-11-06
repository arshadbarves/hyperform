# Permissions and safety

Every action the agent takes (reading a file, editing, running a command,
fetching a web page, calling an MCP tool, starting a subagent) passes
through one permission gate. This page explains the permission modes, the
approval card, how to write allow/ask/deny rules, which paths always ask,
workspace trust, and the optional sandbox.

## Permission modes

| Mode (label in the app) | Setting value | Behavior |
|---|---|---|
| **Ask** | `default` | Asks before edits and before commands that aren't recognised as read-only. |
| **Auto-accept edits** | `acceptEdits` | File edits inside the project (and added folders) run without asking. Simple file commands such as `mkdir`, `touch`, `cp`, `mv`, `rm` and `sed -i` inside the project also run. Other commands still ask. |
| **Bypass** | `bypass` | Everything runs without asking, except what a **deny** rule forbids. |

Switch modes:

- The permission chip in the composer's bar lists all three with a short
  description. Choosing **Bypass** shows a warning; **Turn on Bypass**
  confirms it.
- The command palette's **Switch to …** entries (every mode but Bypass).
- `/mode acceptEdits` (or `default`, `bypass`) from the composer.
- **Settings → Permissions → Permission mode** sets the mode new chats
  start in (`permissions.mode`).

The permission chip, the palette and `/mode` change only the current chat.

### Plan mode

[Plan mode](05-plan-mode-questions-and-todos.md) is a separate switch on
top of the permission mode: while it is on, anything that changes files or
state is refused (including commands HyperForm can't prove are read-only,
such as `cargo test`), and everything else follows the permission mode.
So plan mode with Bypass researches without asking (reading files outside
the project, the web, read-only MCP tools) and still changes nothing.
Turn it on or off with **Shift+Tab**, the composer's **+** menu (**Plan
first** / **Stop planning**), the palette, or `/mode plan`; while it is
on, a **Plan** chip sits beside the permission chip and its **×** turns it
off. **Build** on a plan turns it off and keeps the permission mode.
**Settings → Permissions → Start new chats in plan mode**
(`permissions.planning`) starts every new chat with it on.

[Subagents](04-agents.md) run in the mode their definition sets, or else
in the mode of the agent that started them. Bypass is the exception: while
the chat is in Bypass, every subagent bypasses too, whatever its own mode,
and deny rules still apply. No other chat mode overrides a subagent's own
mode. Plan mode is inherited and can't be turned off from below: while the
chat plans, every subagent plans, and an agent whose definition plans (the
built-in `explore`, `plan` and `review`) plans even under Bypass.

> **Warning:** Use Bypass only in a sandbox or a disposable checkout. The
> agent and its subagents can then delete files or run any command without
> asking.

## What runs without asking

With no rules configured, this is what each mode does, and what changes
while plan mode is on (at any permission mode):

| Action | Ask | Auto-accept edits | Bypass | Plan mode on |
|---|---|---|---|---|
| Read, search or list files in the project or added folders | Allow | Allow | Allow | As the mode |
| Read files elsewhere (for example `/etc/hosts`) | Ask | Ask | Allow | As the mode |
| Edit files in the project | Ask | Allow | Allow | Deny |
| Edit files outside the project | Ask | Ask | Allow | Deny |
| Edit protected paths (see below) | Ask | Ask | Allow | Deny |
| Read-only commands (`git status`, `ls -la`) | Allow | Allow | Allow | As the mode |
| Other commands (`cargo test`, `npm install`) | Ask | Ask | Allow | Deny |
| Fetch a web page / search the web | Ask | Ask | Allow | As the mode |
| MCP tools | Ask | Ask | Allow | As the mode (read-only tools) or Deny |
| Start a subagent, load a skill, update todos | Allow | Allow | Allow | As the mode |

Subagents are allowed because each of *their* actions goes through the same
gate, in the subagent's own mode (Bypass when the chat is in Bypass). A
command counts as read-only only when HyperForm can prove it: every
part of the command line is a known read-only program and every file it
reads is inside the project. You can turn this off with **Run read-only
shell commands without asking** (`permissions.auto_allow_read_only_bash`).

## Answering approval cards

An approval card takes the place of the composer's text box (your draft
comes back once you answer, and Send turns into Stop meanwhile) and the
keyboard focus, unless you are typing in another field. It asks a
question, such as **Allow Bash to run cargo test?**, says why approval is
needed, and previews the diff or command (longer than six lines, it folds
behind **Show all N lines**).

| Button | Key | Effect |
|---|---|---|
| **Allow once** | `y` | Runs this action only. |
| **Always allow…** → **In this chat** | `s` | Runs it and adds the suggested rule for the rest of this chat. |
| **Always allow…** → **In this project** | `p` | Also saves the rule to `.z-engine/settings.local.toml` (your personal, git-ignored project settings). |
| **Deny…** | `n` | Opens a box for optional feedback; Enter or **Deny** sends it, Esc or **Cancel** goes back. The agent receives "The user denied this action" plus your feedback. |

The keys work while the card has keyboard focus, and their hints appear on
the card then; click the card if needed. The **Always allow…** menu shows
the rule it offers, for example `Bash(npm run test:*)`, `Edit`,
`WebFetch(domain:docs.rs)` or `mcp__github__create_issue`.

- **Always allow…** appears only when there is a rule to suggest.
- **In this project** is missing for targets outside the project (for
  example reading `/etc/hosts`): those can be allowed for this chat only.
- Protected paths never offer a rule.

When several requests wait (the agent made several calls at once, or also
asked a question), the oldest approval shows first, then the others one by
one; **N more waiting after this** counts them. Cards from subagents are
labelled with the agent's type and task. Approvals waiting in other chats
are listed in the [Inbox](02-everyday-use.md#the-inbox), where **Allow
once** and **Deny** answer them without opening the chat.

### Approvals from experimental features

Some [experimental decision features](16-decision-features.md#safety) add
cards for calls your rules already allow: **Risk review** ("Risk review:
this Bash call …"), **Secret screening** ("Send 2 possible secrets from
Read output to the model?"; Deny replaces them with
`[secret withheld by the user: <kind>]`), your own
[decision rules](17-decision-settings.md#custom-rules), and the **Loop
guard** after repeated reminders. They offer no **Always allow…** rule. A
decision can only add an approval, a notice or a note; it never allows or
denies anything. In Bypass mode they post a notice instead of asking.

With [work plans](18-work-plans.md#helpers-on-tasks) on, a helper sent
for a plan task asks before it writes outside the task's declared files
(`Edit`, `Write`, `MultiEdit`, or a shell redirect), even when your rules
would allow the write: "This helper works on task t2 and would write
outside the task's declared scope: docs/x.md". A denial stays a denial,
Bypass mode doesn't ask, and each such file is noted on the task either
way.

## Writing rules

Rules live in `[permissions]` in any settings file, or in **Settings →
Permissions**. Rules from every file combine.

```toml
[permissions]
allow = ["Bash(npm test:*)", "Bash(git diff:*)", "Edit(src/**)"]
ask   = ["Bash(git push:*)"]
deny  = ["Read(./.env)", "Read(~/.ssh/**)"]
```

### How a decision is made

1. A matching **deny** rule refuses the action, in every mode.
2. **Plan mode**, when on, refuses anything that changes state, including
   commands that can't be shown to be read-only.
3. **Bypass** mode allows everything else.
4. A matching **ask** rule asks, even if an allow rule or the mode would
   allow it.
5. A matching **allow** rule (from settings, or granted during this chat)
   allows it.
6. Otherwise the defaults above apply.

For a shell command line, a deny or ask rule matches if it matches *any*
command in the line, including commands inside `$(...)` and commands run
through wrappers. An allow rule only helps if *every* command in the line
is covered (or is read-only).

### Rule syntax

A rule is `Tool` or `Tool(specifier)`.

| Rule | Matches |
|---|---|
| `Bash` | Every shell command. |
| `Bash(git status)` | Exactly `git status`. |
| `Bash(npm test:*)` | `npm test` followed by anything: `npm test`, `npm test --watch`. Whole words only, so not `npm testing`. |
| `Bash(cargo test*)` | Same as `Bash(cargo test:*)` (older spelling). |
| `Read` | Reading any file in the project and added folders. |
| `Read(src/**)` | Reading anything under `src/` in the project. |
| `Read(./.env)` | The `.env` file at the project root. |
| `Read(.env)` | Any file named `.env`, at any depth in the project. |
| `Read(~/.ssh/**)` | Anything under `.ssh` in your home folder. |
| `Read(//etc/**)` | Anything under the absolute path `/etc`. |
| `Edit` | Every file change in the project and added folders. |
| `Edit(docs/**)` | Changes under `docs/`. |
| `Grep(src/**)`, `Glob(...)`, `Write(...)`, `MultiEdit(...)`, `NotebookEdit(...)`, `LSP(...)` | That one tool, for those paths. |
| `WebFetch` | Fetching any URL. |
| `WebFetch(domain:docs.rs)` | `docs.rs` and its subdomains (`api.docs.rs`). `domain:*.docs.rs` means the same. |
| `WebSearch` | Web searches. |
| `mcp__github` or `mcp__github__*` | Every tool of the MCP server named `github`. |
| `mcp__github__create_issue` | One MCP tool. |
| `Agent(explore)` (or `Task(explore)`) | Starting that type of subagent. |
| `Skill(deploy)` | Loading that skill. |
| `TodoWrite`, `JobKill`, `Verify`, ... | That tool, by exact name. |

Details for path rules:

- Paths are *gitignore-style* patterns: `*` matches within one folder
  level, `**` across levels. A pattern without a `/` inside it matches at
  any depth; a pattern that matches a folder also covers everything in it.
- A plain path is relative to the project root; `~/` is your home folder;
  `//` starts an absolute path. A single leading `/` is tried both as an
  absolute path and relative to the project root.
- `Read(...)` rules cover every reading tool (Read, Glob, Grep, LSP), and
  also files named in shell commands (so `deny = ["Read(./.env)"]` also
  stops `cat .env`). `Edit(...)` rules cover every writing tool.
- A bare `Edit` or `Read` rule only covers the project and added folders.
  To allow something outside, name it: `Edit(//tmp/**)`.
- On macOS and Windows, path matching ignores upper/lower case.
- Negated patterns (`!src/**`) are not supported.

Rules that don't parse are skipped and reported in a notification; the
other rules still apply.

### Examples

```toml
[permissions]
# Let tests, builds and linters run without asking
allow = ["Bash(npm test:*)", "Bash(npm run lint:*)", "Bash(cargo build:*)",
         "Bash(cargo test:*)", "WebFetch(domain:developer.mozilla.org)"]

# Always confirm anything that leaves your machine
ask = ["Bash(git push:*)", "Bash(npm publish:*)", "mcp__github__merge_pull_request"]

# Never touch secrets or production
deny = ["Read(./.env)", "Read(.env.*)", "Read(~/.ssh/**)", "Read(~/.aws/**)",
        "Bash(rm -rf:*)", "mcp__prod_db", "Agent(general)"]
```

## Extra folders

By default the agent may work only inside the project folder. To let it
read and edit another folder too:

- add it under **Settings → Permissions → Folders → Additional
  directories** (`permissions.additional_directories`), or
- run `/add-dir ../shared-lib` for this chat, or
  `/add-dir ../shared-lib --save` to also save it to your personal project
  settings.

## Protected paths

These locations always ask before a change, even in Auto-accept edits mode
and even when an allow rule matches, because writing them could run code
or change permissions:

- any `.git` folder, at any depth;
- the `.z-engine/` folder at the project root (settings, agents, commands);
- `.claude/settings.json`, `.claude/settings.local.json` and `.mcp.json`.

Bypass mode allows them; only a deny rule stops them there.

## Workspace trust

A project can ship its own settings (`.z-engine/settings.toml` or
`settings.local.toml`), agents, and commands. Some of those could run
programs on your machine or loosen your permissions, so until you trust the
workspace an untrusted project can only make things **stricter**. When you
open a chat in a project that sets any of them, a **Trust this workspace?**
banner lists what it sets; choose **Trust this workspace** or **Not now**.

Until you trust it, these project settings are ignored and yours are used
instead:

| Ignored from the project | Why |
|---|---|
| hooks, MCP servers, checks | they run programs |
| permission `mode`, `allow` rules, `additional_directories`, `auto_allow_read_only_bash` | they would approve actions for you |
| everything under `[shell]` (shell path, environment, sandbox) | it decides how commands run |
| everything under `[provider]` | it decides where your code is sent |
| everything under `[web]` | it controls web access, including private networks |
| everything under `[lsp]` | language server commands are programs |
| everything under `[decisions]`, including `[[decisions.rules]]` | it decides where decision questions are sent, its sidecar command is a program, and its rules can add approvals |
| `work.merge = "auto"` (the banner says "automatic merges") | it would bring helpers' changes into your files without asking |
| removing your `work.serial_paths` (the banner says "fewer serial paths") | it would let tasks that touch lockfiles and manifests run side by side; paths the project adds still apply |

The project's `deny` and `ask` rules **do** apply, because they only add
caution, and so does plan mode (`permissions.planning = true`, or the
older `mode = "plan"`). Model choices, context, verification mode and
appearance also apply. In addition:

- A project's custom agent can't run in a looser permission mode than the
  agent that started it.
- A project's custom command gets nothing from its `allowed-tools`: its
  inline `` !`command` `` lines are not run and it grants no extra
  permissions for its turn.
- Automatic verification checks don't run.

Trust details:

- Trust is stored in `trust.json` in your config folder, per project
  folder. Subfolders and worktrees are separate folders and need their own
  trust.
- Change it later under **Settings → Advanced → Workspace → Trust this
  workspace**. The first-run setup and the project home's **Set up this
  project** card can trust a project too.
- Your *user* settings, agents and commands always apply.

## The sandbox

The optional sandbox confines shell commands at the operating-system level.
Turn it on in **Settings → Advanced → Shell → Sandbox commands**, or:

```toml
[shell.sandbox]
enabled = true
allow_network = false        # false: only localhost is reachable
extra_writable = ["~/scratch"]
auto_allow = true            # run sandboxed commands without asking
```

When it is on, commands the agent runs (Bash, background shells and
verification checks) may **write only** to:

- the project folder (for an agent in a worktree: its worktree) and the
  additional directories;
- the temporary folder;
- tool caches that exist: `~/.cargo/registry`, `~/.cargo/git`, `~/.npm`,
  `~/.cache`, `~/.gradle`, `~/.m2`, and `~/Library/Caches` on macOS;
- `extra_writable` entries (`~/` is home; relative paths start at the
  project).

Everything else stays readable but not writable. Inside the project,
`.git/hooks`, `.git/config`, `.z-engine/settings.toml`,
`.z-engine/settings.local.toml`, `.claude/settings.json`,
`.claude/settings.local.json` and `.mcp.json` stay read-only.

- **Network:** with `allow_network = false`, all connections except to
  localhost are blocked (including package downloads). On Linux the command
  gets its own network, so even servers on your machine's localhost are out
  of reach.
- **Auto-allow:** with `auto_allow = true`, commands that would otherwise
  ask run without asking when every file they name for writing stays
  inside the writable folders. Deny rules, ask rules and plan mode still
  apply.
- **Platforms:** macOS uses the built-in `sandbox-exec`. Linux needs
  bubblewrap (`bwrap`) installed. Windows has no sandbox yet. When no
  sandbox is available, a notice says why, commands run unconfined, and
  they keep asking for approval.

> **Note:** `!` commands you type yourself and hook scripts are not
> sandboxed.

If a sandboxed command fails because it tried to write elsewhere, the
agent is told so and usually adjusts.

See also: [Settings reference](12-settings-reference.md#permissions) · [Hooks](08-hooks.md) · [Agents](04-agents.md)
