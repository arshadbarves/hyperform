# Memory and context

The agent doesn't remember anything between chats on its own. What it knows
comes from its *context*: the instructions, files and conversation sent to
the model with every request. This page explains instruction files
(`AGENTS.md`, `CLAUDE.md`), rules, how the context window fills up, how
compaction keeps long chats going, the repository map, and prompt caching.

## Instruction files

Instruction files are plain markdown files with standing guidance: how to
build and test, code conventions, things to avoid. Their contents are part
of every request.

```markdown
# AGENTS.md
- Build with `pnpm build`; run a single test with `pnpm vitest run <file>`.
- API handlers live in `packages/api/src/routes/`; never edit `generated/`.
- Use conventional commit messages.
```

HyperForm reads these files, lowest precedence first (later files win when
they disagree):

| Scope | Files |
|---|---|
| **User** (all projects) | `~/.claude/CLAUDE.md`, then `AGENTS.md` in your config folder (`~/.config/z-engine/AGENTS.md`; Windows: `%APPDATA%\z-engine\AGENTS.md`) |
| **Project** | In each folder from the repository root down to the project folder: `CLAUDE.md`, then `AGENTS.md` |
| **Local** (just you) | `CLAUDE.local.md`, then `AGENTS.local.md` in the project folder |
| **Nested** | `CLAUDE.md` and `AGENTS.md` in subfolders of the project |

- The walk up from the project folder stops at the folder that contains
  `.git`, and never includes your home folder.
- `CLAUDE.md` files are read only while Claude compatibility is on
  (**Settings → Advanced → Workspace → Read .claude folders**, the
  `compat.claude` setting, on by default). At each level `AGENTS.md` comes
  after `CLAUDE.md`, so it wins. A `CLAUDE.md` that is a link to
  `AGENTS.md` is read once.
- **Nested** files are added the first time the agent reads or writes a
  file in or below their folder, and they take precedence for work there.
- Each file is cut at 64 KiB. Empty files are ignored. Project files that
  are links pointing outside the project are skipped.
- Instructions guide the agent but can't grant permissions; approvals still
  apply.

> **Tip:** Run `/init` to have the agent write or improve `AGENTS.md` from
> your build files, CI configuration and code.

Edit the three main files in **Settings → Memory** (User, This project,
Personal). The page also lists other instruction files it found, such as
`CLAUDE.md` or files in parent folders. `AGENTS.local.md` is meant for you
alone; add it to your `.gitignore`.

### Save a note with `#`

Type `# ` followed by a note in the composer and pick **Project memory**,
**Personal project memory** or **User memory**. The note is appended to
`AGENTS.md`, `AGENTS.local.md` or your user `AGENTS.md` as a bullet, and
the chat's instructions reload. `/memory` starts such a note;
`/remember project <text>` (or `local`, `user`) does it in one command.

## Rules

Rules attach guidance to specific files, so it's only in context when
relevant. Put markdown files under `rules/` (subfolders allowed) in
`<project>/.z-engine/rules/` or `~/.config/z-engine/rules/`, or create them
in **Settings → Agents & Commands → Rules**.

```markdown
---
description: Conventions for React components
globs: src/components/**/*.tsx, src/hooks/**/*.ts
alwaysApply: false
---

- Function components only; props interface named `<Component>Props`.
- Co-locate tests as `<Component>.test.tsx`.
```

| Field | Meaning |
|---|---|
| `name` | Defaults to the file's path under `rules/` (subfolders joined with `:`). |
| `description` | What the rule covers. |
| `globs` | File patterns (gitignore style, relative to the project; a pattern without `/` matches at any depth). A list or a comma-separated string. |
| `alwaysApply` | `true` puts the rule in every request, like an instruction file. |

A rule with `globs` joins the context the first time the agent reads or
writes a matching file, once per agent. A rule with neither `globs` nor
`alwaysApply: true` is never used. Rules are not read from `.claude/`
folders.

## Output styles

An output style changes how the agent writes its answers (tone, structure,
length). Put a markdown file in `output-styles/` in your config folder or
the project's `.z-engine/` folder:

```markdown
---
name: terse
description: Short answers, bullet points, no preamble
---

Answer in at most five bullet points. Skip introductions and summaries.
```

Choose it under **Settings → Appearance → Response style**
(`ui.output_style = "terse"`). Its text is added to the system prompt.

## The context window

Models read text as *tokens*. A token is a chunk of about four characters,
roughly three-quarters of an English word. The *context window* is how many
tokens a model can take in one request (for example 200,000). Everything
counts: the system prompt, instruction files, tool definitions, the
conversation, and every file or command output the agent has looked at.

The small ring to the right of the island in the title bar shows how full
this chat's context is. Its percentage appears from 65% (amber, red from
85%); hover it for the numbers, such as "Context 42% · 84k of 200k". Click
it, or type `/context`, for the context card:

- the percentage used, the tokens used of the limit, and how many are left;
- a bar split by layer, with a legend: **System prompt**, **Tool
  definitions**, **Instructions**, **Conversation**;
- when compaction starts on its own ("HyperForm compacts older messages by
  itself at 92%.");
- **Compact now** (when the agent is idle) and **Inspect prompt**.

HyperForm takes the window size from the model catalog, preferring the
active provider's own listing (see
[Context window and output length](11-models-providers-and-cost.md#context-window-and-output-length));
set `model.context_window` to override it. Unknown models are assumed to
have 128,000 tokens.

### Inspect the prompt

**Inspect prompt** (in the context card) or **Inspect the prompt** (in the
command palette) opens the side panel's **Context** tab, the prompt
inspector: the last request sent to the model, part by part. Its header
shows the model and **Copy all**.

- **The map** at the top is a ring of the context window: how much the
  request fills (for example "42% of the window", "84k of 200k tokens"),
  split into four kinds: **Instructions** (the system prompt), **Project**
  (`AGENTS.md`, the repository map, saved notes), **Conversation**
  (messages and tool results) and **Tools** (tool definitions). Click a
  kind to show only its parts; click it again to show all.
- **The outline** below lists the parts by kind with their token counts;
  conversation parts show their first line. **Search the request** also
  searches the text inside the parts, and ↑/↓ move through them.
- **Insights**, folded at the bottom, names the largest part, how many
  tokens are reusable across turns (system prompt and tools, which the
  provider can cache) and how many change every turn, with hints to shrink
  the request. Below it, a **Decisions** section appears while an
  [experimental decision feature](15-experimental-features.md#see-what-decisions-did)
  runs, or has run, in the chat. With a [work plan](18-work-plans.md)
  attached, a **Task packet** section shows what the agent was given
  about its task (when and why it was sent), part by part, with why each
  notebook entry made it in and what was left out for the budget.
- **The selected part** reads on the right while the panel uses the whole
  stage (picking a part in the docked panel expands it), with its kind,
  tokens and share of the request. **Reader** shows it formatted (a tool as
  its description and input schema); **Raw** shows the exact text with line
  numbers and a **Wrap lines** switch. A button copies the part.

Esc docks the panel beside the chat again, then closes it.

## Automatic compaction

Long sessions are trimmed automatically, in two stages:

1. **Above half the window**, older tool results are cleared from the
   conversation the model sees. The most recent ones stay
   (`context.keep_recent_tool_results`, default 8), and short results (under
   1,000 characters) are never cleared. The full output is saved in the
   session's artifacts folder and the agent is told where.
2. **Above `context.compact_at_percent`** (default 92%, allowed 50–99), the
   fast model writes a summary of the older history, which replaces it. The
   last few messages are kept word for word where possible, and a tool call
   is never separated from its result.

While the summary is written, **Compacting context…** shows at the end of
the transcript. A **Context compacted · 180k → 24k** divider then marks
the spot; click its **Summary** to read the summary. Your transcript keeps
everything; compaction only affects what is sent to the model. The todo
list survives compaction. If the provider reports that a request is too
long, HyperForm compacts and retries once.

With a [work plan](18-work-plans.md) attached, the
summary keeps the plan short, since the plan is stored outside the
conversation. Facts the summary lists are added to the plan's notebook as
unconfirmed discoveries, and the agent's task packet follows the summary;
the divider then adds **· plan restored**.

You can compact yourself when the agent is idle with `/compact`, **Compact
now** in the context card, or **Compact the conversation** in the command
palette. `/compact` takes optional focus instructions:

```text
/compact keep the details of the database migration decisions
```

A `PreCompact` [hook](08-hooks.md) runs before each summary.

### Experimental: smarter context

Several [experimental decision features](16-decision-features.md#context-and-cost)
change what the model is sent, never your transcript:

- **Relevance-aware compaction** clears the old tool results the current
  task no longer needs first.
- **Task-scoped history** sets earlier exchanges aside when you start a
  new task in a long chat. A divider says "N earlier exchanges (X tokens)
  set aside for this task"; click **Include full history** to bring them
  back for this task.
- **First-request context** orders the repository map for the chat's
  first request, and **File prefetch** attaches files the request will
  likely need.
- **Relevant output trimming** and **Search ranking** keep the parts of
  long command output and capped search results that matter to the task.

## The repository map

To help the agent find its way, HyperForm adds a compact outline of your
project to the system prompt: the main definitions (functions, types,
classes) of Rust, TypeScript/JavaScript, Python and Go files, ranked by how
often they are referenced and by the files touched in this chat (with a
[work plan](18-work-plans.md), also the files in the
scope of the tasks being worked on).

- It is built in the background when a chat opens and rebuilt only after a
  compaction or a settings change, so the start of the prompt stays stable
  for caching.
- Size: `context.repo_map_chars` (default 6,000 characters). Up to 400
  files are considered; files over 200 KiB are skipped.
- Turn it off with `context.repo_map = false` (**Settings → Advanced →
  Context → Repository map**).

The system prompt also includes a snapshot taken when the chat opens: the
project folder, platform, shell, date, model, and (in a git repository) the
branch, `git status` and recent commits.

## Prompt caching

Much of each request is the same as the previous one: the system prompt,
instructions, tool definitions and earlier conversation. With *prompt
caching*, the provider keeps that unchanged beginning and charges much less
for reading it again (cache reads are typically a fraction of the normal
input price), and it answers faster.

HyperForm orders the prompt so the stable parts come first and marks cache
points on the system prompt, the tool list and the last two user messages.

- **Anthropic:** on by default.
- **OpenRouter:** on by default (passed through to models that support it).
- **Other providers:** off by default; turn it on per provider in
  **Settings → Providers → Configure → Endpoint and advanced options →
  Prompt caching** (`provider.cache_control`).

Cache reads and writes are counted separately in `/cost`. See
[Models, providers and cost](11-models-providers-and-cost.md#prompt-caching-and-cost).

See also: [Commands and skills](06-commands-and-skills.md) · [Settings reference](12-settings-reference.md#context) · [Hooks](08-hooks.md) · [Decision features](16-decision-features.md)
