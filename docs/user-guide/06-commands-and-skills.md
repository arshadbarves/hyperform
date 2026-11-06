# Commands and skills

Slash commands are shortcuts you type in the composer, starting with `/`.
Some are answered by HyperForm directly, some send a prepared prompt to the
agent, and some control the app. This page lists every built-in command,
covers the command palette, shows how to write your own commands, explains
MCP prompts, and covers skills and the `#` memory shortcut.

Type `/` to see the list. It's grouped into **Prompts** (sent to the
model), **Session** (answered by HyperForm without the model) and **App**
(handled by the window). Each entry shows where it comes from: BUILT-IN,
APP, USER, PROJECT or MCP. `/help` prints all commands and keyboard
shortcuts in the chat.

## Session commands

These are answered immediately, without using the model.

| Command | What it does |
|---|---|
| `/compact [instructions]` | Summarizes older history to free context. Optional text tells the summary what to focus on. Only when the agent is idle. |
| `/cost` | Shows this chat's cost and token counts (input, output, cache read, cache write), plus tokens per agent when subagents ran. |
| `/status` | Shows the session id, project and its trust state, model and fast model, provider, permission mode, effort, context use, whether code checkpoints are on, and running background jobs. |
| `/model [id]` | Without an id, shows the current model. With an id, switches this chat's model. |
| `/mode <default\|acceptEdits\|bypass\|plan>` | Sets this chat's permission mode; `/mode plan` turns [plan mode](05-plan-mode-questions-and-todos.md) on or off instead. Either way it replies with both. |
| `/effort <low\|medium\|high\|max\|default>` | Sets this chat's reasoning effort; `default` sends none. |
| `/mcp` | Shows each MCP server's state, tool/resource/prompt counts, and errors with the end of its error output. |
| `/todos` | Shows the current todo list. |
| `/replan [what to reconsider]` | Listed only while [work plans](18-work-plans.md) is on. Checks the chat's plan against the workspace. When something no longer fits, or you say what to reconsider, the plan goes to Replanning and a turn starts: the agent revises the tasks without changing code and asks you to accept the changes in the **Plan** tab. Otherwise it replies that there is nothing to replan. |
| `/doctor` | Checks the provider and key, model, catalog, git, ripgrep, language servers, MCP servers, hooks, workspace trust and verification. |
| `/add-dir <path> [--save]` | Lets the agent work in another folder for this chat; `--save` also stores it in your personal project settings. |
| `/remember <project\|local\|user> <text>` | Appends `- text` to an instruction file (see [below](#save-a-note-with-)). |

## Prompt commands

These send a prepared prompt to the agent, so they start a turn.

| Command | What it does |
|---|---|
| `/init [focus]` | Analyzes the repository and creates or improves `AGENTS.md` with build/test commands, architecture, conventions and gotchas. |
| `/review [target]` | Reviews uncommitted changes, or the files, commit, range or branch you name, using `review` agents. Reports findings by severity; doesn't change code. |
| `/security-review [target]` | A security-focused review of pending changes. |
| `/commit [hint]` | Commits the current changes in the repository's style. For that turn it may run `git status`, `git diff`, `git log`, `git add` and `git commit` without asking. Never pushes. |

The composer shows your command as a chip in the transcript; the full
instructions go to the model.

## App commands

| Command | What it does |
|---|---|
| `/help` | Lists commands and keyboard shortcuts. |
| `/agents` | Opens the side panel's **Agents** tab on its helpers. |
| `/jobs` | Opens the side panel's **Agents** tab on its **Jobs**. |
| `/context` | Opens the [context card](07-memory-and-context.md#the-context-window): how full the context is, by layer, with **Compact now** and **Inspect prompt**. Without an open chat, it prints the per-layer breakdown in a new chat instead. |
| `/permissions` | Opens Settings → Permissions. |
| `/hooks` | Shows the recent hook runs of this chat. |
| `/memory` | Starts a `#` memory note in the composer. |
| `/config` | Opens Settings at **Models**. |
| `/resume` | Opens the palette listing your chats. |
| `/export [markdown\|json]` | Copies the transcript to the clipboard. |
| `/clear` | Starts a new chat (the old one is kept). |

Commands that prompt the model can't be queued while the agent is working;
you'll see "/name waits for an idle session". An unknown command shows a
suggestion ("Did you mean /review?").

## The command palette

Press ⌘K (Ctrl+K on Windows and Linux), or click **Search** in the sidebar,
to open the command palette. One search field finds actions, chats,
projects and settings. Before you type, it lists:

| Group | Entries |
|---|---|
| **Actions** | **New chat**, **New chat in a worktree…**, **Add a project…** |
| **Go to** | **Home**, **Inbox**, **Settings**, **Show or hide the sidebar** |
| **Recent chats** | Your six latest chats |
| **This chat** | **Review changes**, **Plan**, **Agents**, **Background jobs** and **Inspect the prompt** (each opens its side panel tab), **Context usage**, **Compact the conversation**, **Copy the chat as Markdown**, **Copy the chat as JSON** |
| **Pet** | **Rename Zen**, **Change Zen's look** (both open **Settings → Pet** at that field), **Show Zen's card**, and at the **Lively** level **Stop Zen roaming** (or **Let Zen roam**) and **Call Zen back**, with your pet's name; none while the pet is off |
| **Permission mode** | **Switch to …** each mode except the current one and Bypass |
| **Help** | **Commands and shortcuts** (the same as `/help`) |
| **Projects** | Your project folders; picking one makes it the active project |

Once you type, single settings join the list under **Settings**, found by
their name or by everyday words ("budget" finds **Session cost cap**, "api
key" finds **Providers and API keys**); picking one opens Settings at that
row. Names that start with what you typed rank first, then names with a
word that starts with it, then names that contain it, then related
keywords; letters typed in order also match ("nc" finds **New chat**).
↑ and ↓ move, Enter opens, Esc closes.

## Custom commands

A custom command is a markdown file whose body is a prompt template. Put
it in a `commands/` folder:

| Location | Scope |
|---|---|
| `<project>/.z-engine/commands/` | This project (highest precedence) |
| `<project>/.claude/commands/` | This project, Claude Code format |
| `~/.config/z-engine/commands/` (Windows: `%APPDATA%\z-engine\commands\`) | All your projects |
| `~/.claude/commands/` | All your projects, Claude Code format |

The file name becomes the command: `commands/changelog.md` is `/changelog`,
and subfolders use `:`, so `commands/frontend/component.md` is
`/frontend:component`. A custom command can replace a prompt command like
`/review`, but not a session command like `/status`. You can also create
commands in **Settings → Agents & Commands → Commands**.

### Example

`.z-engine/commands/fix-issue.md`:

```markdown
---
description: Fix a GitHub issue by number
argument-hint: <issue-number> [notes]
allowed-tools: Bash(gh issue view:*), Bash(npm test:*)
model: anthropic/claude-sonnet-4.5
---

Fix issue #$1. Extra notes from me: $2

Issue details:
!`gh issue view $1`

Our contributing guide:
@CONTRIBUTING.md

Run the tests before you finish.
```

Run it with `/fix-issue 128 "keep the old API"`.

### Frontmatter

| Field | Meaning |
|---|---|
| `description` | Shown in the command menu. Defaults to the first line of the body (up to 100 characters). |
| `argument-hint` | Shown after the name in the menu, for example `<issue-number> [notes]`. |
| `allowed-tools` | Permission rules granted **for that turn only**, as a list or a comma-separated string, for example `Bash(git add:*)`. Ignored for commands from an [untrusted](03-permissions-and-safety.md#workspace-trust) project. |
| `model` | Model for that turn only. `inherit` or empty keeps the chat's model. |

### Placeholders in the body

| Placeholder | Replaced by |
|---|---|
| `$ARGUMENTS` | Everything you typed after the command name. |
| `$1` … `$9` | The individual arguments, split like a shell does: `"two words"` is one argument. Missing ones become empty. |
| `@path` | The contents of that file (relative to the project root), added below the prompt in a fenced block. Only text files the permission rules let the agent read; up to 256 KiB per file and 1 MiB in total. |
| `` !`command` `` | The command's output, run in the project root before the prompt is sent. It runs only if the command's `allowed-tools` or your permission rules allow it without asking; otherwise a note says it needs approval. Limit: 2 minutes, 64 KiB of output. |

Placeholders are filled in before inline commands run, so `` !`gh issue view $1` ``
works. If the body contains neither `$ARGUMENTS` nor `$1`–`$9` and you typed arguments, they are
appended as `ARGUMENTS: ...` so they're never lost.

## MCP prompts as commands

MCP servers can offer *prompts*: templates with named arguments. Each
prompt of a running server appears as `/mcp__<server>__<prompt>`. Pass
arguments in order, or as `name=value`:

```text
/mcp__github__summarize_pr 482
/mcp__github__summarize_pr focus=tests number=482
```

If a required argument is missing, a notice tells you which one instead of
starting a turn. See [MCP and code intelligence](09-mcp-and-code-intelligence.md).

## Skills

A *skill* is a folder of instructions (and optionally scripts or reference
files) for one kind of task, which the agent loads only when it needs it.
The skill names and descriptions are listed in the agent's system prompt;
when a request matches, the agent loads the full instructions with the
`Skill` tool and follows them.

Create `skills/<name>/SKILL.md` in any of these folders:
`<project>/.z-engine/skills/`, `<project>/.claude/skills/`,
`~/.config/z-engine/skills/`, or `~/.claude/skills/`.

```markdown
---
name: release-notes
description: Write release notes from the git log since the last tag. Use when the user asks for release notes or a changelog entry.
---

# Release notes

1. Find the last tag with `git describe --tags --abbrev=0`.
2. Read `git log <tag>..HEAD --oneline`.
3. Group changes into Added, Changed and Fixed, following `template.md`
   in this folder.
```

| Field | Meaning |
|---|---|
| `name` | Defaults to the folder name. |
| `description` | **Required.** When to use the skill; the agent decides from this. |

When loaded, the agent is also told the skill's folder, so it can use
files next to `SKILL.md` by relative path. You can ask for a skill by name
("use the release-notes skill"). Loading skills is allowed by default; a
rule like `deny = ["Skill(deploy)"]` blocks one.

## Save a note with `#`

Start a message with `#` to save a standing instruction without asking the
model:

```text
# Always run `pnpm test --filter api` for changes under packages/api.
```

A small menu lets you pick where it goes (Enter picks the highlighted one):

| Target | File |
|---|---|
| **Project memory** | `AGENTS.md` in the project root (shared with your team) |
| **Personal project memory** | `AGENTS.local.md` in the project root (just you) |
| **User memory** | `AGENTS.md` in your config folder (all projects) |

The note is appended as a bullet and the chat reloads its instructions. `##`
at the start is a markdown heading, not a note. `/remember` does the same
from a command. More in [Memory and context](07-memory-and-context.md).

See also: [Memory and context](07-memory-and-context.md) · [Agents](04-agents.md) · [Permissions and safety](03-permissions-and-safety.md)
