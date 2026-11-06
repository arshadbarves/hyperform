# Glossary

Part of [How HyperForm works](README.md). Short, plain-language definitions
of the terms used on these pages, in alphabetical order. Each entry links
to the section that explains the idea in full. The names of the window's
parts (Inbox, island, pet, side panel) are in
[Glossary: the desktop app](glossary-desktop.md); the terms of
experimental features, the decision layer and work plans are in
[Glossary: experimental features](glossary-experimental.md).

## A

### Agent

The model working in a loop with tools: it reads code, runs commands and
edits files for you, then reports what it did. The agent you chat with is
the *main agent*. See [What an AI coding agent is](README.md#what-an-ai-coding-agent-is).

### API key

A secret string that lets HyperForm use your account at a
[provider](#provider). Keys are kept in `auth.json` in your config folder
or read from environment variables, and each is sent only to its own host.
See [Providers and models](features-integrations-and-safety.md#providers-and-models).

### Approval

A card that asks you before an action runs, with **Allow once**, **Always
allow…** (in this chat or in this project) and **Deny…**. It appears in
the composer, in place of the text box, when the permission check answers
*ask*. See [Permissions](features-core.md#permissions-modes-rules-and-approvals).

## B

### Background agent

A [subagent](#subagent) started with `run_in_background: true`. It runs as
a background job while the main agent keeps working, survives **Esc**, and
sends a reminder when it finishes. See [Background jobs](features-agents-and-context.md#background-jobs).

## C

### Check

A project command that proves something about the code: a test, build,
typecheck, lint or format run. Checks are found by reading build files or
set in settings, and every run is recorded as evidence. See [Verification](features-integrations-and-safety.md#verification).

### Checkpoint

A snapshot of your project's files, taken before each of your messages and
stored in a hidden git repository outside the project. [Rewind](#rewind)
uses it to put files back. See [Checkpoints and rewind](features-integrations-and-safety.md#checkpoints-and-rewind).

### Command (protocol)

A typed request the app sends to the engine, such as `submit` (send a
message), `steer`, `resolveApproval` or `cancel`. Commands go in and
[events](#event) come out. See [hyperform-protocol](crates.md#hyperform-protocol).

### Compaction

Replacing the older part of a long conversation with a summary so it fits
the [context window](#context-window) again. It happens automatically near
the limit, or when you run `/compact`. See [Compaction](features-agents-and-context.md#compaction).

### Context window

The most text, counted in [tokens](#token), that a model can read in one
request: instructions, tool list and conversation together. Unknown models
are assumed to have 128,000 tokens. See
[The context window](features-agents-and-context.md#the-context-window-and-token-estimates).

### Crate

A Rust package, the unit the code is split into. HyperForm has one crate per
responsibility, such as `hyperform-host` for everything that touches the
operating system. See [Crates](crates.md).

## E

### Effort and thinking

Some models reason step by step before they answer; that reasoning is
*thinking*, shown as a collapsible section. *Effort* (`low`, `medium`,
`high` or `max`) sets how much thinking to ask for. See
[Providers and models](features-integrations-and-safety.md#providers-and-models).

### Event

A typed message the engine sends to the app when something happens, such
as `textDelta`, `toolStarted` or `turnFinished`. The window is drawn from
events. See [From events to a screen](features-desktop-app.md#from-events-to-a-screen).

## F

### Fallback model

A backup model tried, in order, when a request to the main model still
fails after retries and before any answer text arrived. They are listed in
`model.fallbacks`. See
[Providers and models](features-integrations-and-safety.md#providers-and-models).

### Fingerprint

A short digest of the size and modification time of every project file.
If it changed after a [check](#check) ran, that check's evidence is stale.
See [Verification](features-integrations-and-safety.md#verification).

## H

### Hook

Your own script that HyperForm runs at a fixed moment, such as before a tool
runs (`PreToolUse`) or when the agent wants to stop (`Stop`). A hook can add
context, block an action or send the agent back to work. See [Hooks](features-interaction.md#hooks).

## I

### Instruction file

A markdown file of standing instructions the agent reads in every chat:
`AGENTS.md` and `AGENTS.local.md` in the project (`CLAUDE.md` and
`CLAUDE.local.md` are read too), and your own `AGENTS.md` in the config
folder. See [Instruction files](features-agents-and-context.md#instruction-files).

## L

### Language server

A program that understands one programming language the way a code editor
does: where things are defined, who uses them, what does not compile.
HyperForm starts one when the agent works on a matching file. See
[Language servers](features-integrations-and-safety.md#language-servers).

### LSP

Language Server Protocol, the standard way editors talk to
[language servers](#language-server). It is also the name of the tool the
agent uses to ask them questions. See
[Language servers](features-integrations-and-safety.md#language-servers).

## M

### MCP

Model Context Protocol, an open standard for plugging extra tools, data and
prompts into AI apps. See [MCP servers](features-integrations-and-safety.md#mcp-servers).

### MCP server

A program, started locally or reached at a URL, that offers tools,
resources and prompts over [MCP](#mcp). Its tools appear to the agent as
`mcp__<server>__<tool>`. See
[MCP servers](features-integrations-and-safety.md#mcp-servers).

### Merge-back

Bringing an isolated agent's changes from its [worktree](#worktree) into
your project. **Apply** tries a clean patch, then a three-way merge;
**Discard** throws the changes away. See
[Worktree isolation and merge-back](features-agents-and-context.md#worktree-isolation-and-merge-back).

### Model

The AI program that reads text and writes text, such as Claude or GPT. On
its own it cannot open a file or run a command; the engine does that for
it. See [What an AI coding agent is](README.md#what-an-ai-coding-agent-is).

## P

### Permission mode

The chat-wide setting for how much runs without asking: **Ask**,
**Auto-accept edits** or **Bypass**. [Plan mode](#plan-mode) is a separate
switch on top of it. See
[Permissions](features-core.md#permissions-modes-rules-and-approvals).

### Plan mode

A switch, separate from the [permission mode](#permission-mode), under
which the agent researches and proposes a plan but changes nothing;
**Build** turns it off. Shift+Tab flips it. See
[Plan mode, structured questions and todos](features-interaction.md#plan-mode-structured-questions-and-todos).

### Prompt

Text given to a model to act on. Your message is a prompt, and so are the
instructions the engine writes, which are kept as markdown files in one
crate. See [hyperform-prompts](crates.md#hyperform-prompts).

### Prompt cache

The provider's short-term memory of the unchanged beginning of a request,
so the next request can read it at a lower price instead of paying for it
again. See [Prompt caching](features-agents-and-context.md#prompt-caching).

### Protocol

The shared dictionary of [commands](#command-protocol) and
[events](#event) between the app and the engine, written once in Rust and
generated as TypeScript for the window. See
[One dictionary for both languages](features-desktop-app.md#one-dictionary-for-both-languages).

### Provider

The service that runs the model, such as Anthropic, OpenRouter, OpenAI or
Ollama on your own computer. See
[Providers and models](features-integrations-and-safety.md#providers-and-models).

## R

### Repo map

A short table of contents of your code (the main functions and types, most
important first) added to the [system prompt](#system-prompt) so the agent
knows where to look. See
[The repository map](features-agents-and-context.md#the-repository-map).

### Rewind

Going back to the moment before one of your messages: the code, the
conversation, or both, from **Rewind** at the end of that turn. See
[Checkpoints and rewind](features-integrations-and-safety.md#checkpoints-and-rewind).

### Round

One model response inside a [turn](#turn), together with the tool calls it
asks for and their results. A turn has one or more rounds. See
[Conversations, turns and rounds](features-core.md#conversations-turns-and-rounds).

### Rule

A pattern that allows, asks for or denies an action, such as
`Bash(npm test:*)` or `Read(./.env)`. Rules come from settings or from an
approval's **Always allow…**. See [Permissions](features-core.md#permissions-modes-rules-and-approvals).

### Rule file

A markdown file under a `rules/` folder with extra instructions for the
agent. One with `globs` joins the context only once the agent works with a
matching file. See [Glob-scoped rules](features-agents-and-context.md#glob-scoped-rules).

## S

### Sandbox

An operating-system fence around shell commands: they may write only in the
project and a few safe folders, and can be kept off the network. It is off
by default. See [The sandbox](features-integrations-and-safety.md#the-sandbox).

### Session

One chat: its messages, turns, approvals and checks, stored in its own
folder so it can be reopened and resumed. See [Sessions and persistence](features-integrations-and-safety.md#sessions-and-persistence).

### Skill

A folder with a `SKILL.md` file of instructions for one kind of task. The
agent sees a short list of skills and loads one with the `Skill` tool when
it fits. See
[Slash commands, custom commands and skills](features-interaction.md#slash-commands-custom-commands-and-skills).

### Slash command

A command you type in the composer, starting with `/`, such as `/compact`
or `/model`. Some are built in; custom ones are markdown files, and MCP
prompts appear as `/mcp__<server>__<prompt>`. See
[Slash commands, custom commands and skills](features-interaction.md#slash-commands-custom-commands-and-skills).

### Steering

A message you type while the agent is still working. It does not wait for
the turn to end: it reaches the model with the next tool results, so you
can change course midway. See
[Steering, interrupt and cancel](features-interaction.md#steering-interrupt-and-cancel).

### Stop boundary

The moment the model answers without asking for tools. Before the turn
ends, `Stop` hooks, waiting [steering](#steering) messages and
verification may send it back for another [round](#round). See
[Conversations, turns and rounds](features-core.md#conversations-turns-and-rounds).

### Subagent

A helper agent the main agent starts with the `Agent` tool for one job. It
works with its own clean context and brings back a short report. See
[Subagents](features-agents-and-context.md#subagents).

### System prompt

The standing instructions at the start of every request: base rules, your
environment, [instruction files](#instruction-file), the list of skills
and the [repo map](#repo-map). See
[One turn at a glance](README.md#one-turn-at-a-glance).

## T

### Token

The unit models read and charge by: a piece of text a few characters long
(HyperForm estimates about four characters per token). Context windows and
prices are counted in tokens. See
[The context window](features-agents-and-context.md#the-context-window-and-token-estimates).

### Tool

An action the engine offers the model, with a name and an input format,
such as `Read`, `Edit`, `Bash` or `Grep`. The engine, not the model, carries
it out. See [Tools and tool batches](features-core.md#tools-and-tool-batches).

### Tool call

The part of a model's reply that asks for a [tool](#tool) to run with given
input, for example `Read` on `src/auth.ts`. See
[Tools and tool batches](features-core.md#tools-and-tool-batches).

### Tool result

What goes back to the model after a [tool call](#tool-call): the tool's
output, or why it was denied, failed or cancelled. Every call gets exactly
one result. See [Tools and tool batches](features-core.md#tools-and-tool-batches).

### Transcript

The written record of a conversation: what you see in the chat, and the
file that keeps each subagent's own conversation (`agents/<id>.jsonl`). See
[The transcript and tool cards](features-desktop-screens.md#the-transcript-and-tool-cards).

### Turn

Everything between your message and the agent's final answer, made of one
or more [rounds](#round). It ends with an outcome, its cost and a
[verification badge](#verification-badge). See
[Conversations, turns and rounds](features-core.md#conversations-turns-and-rounds).

## V

### Verification badge

The verdict on a turn's receipt: **Verified**, **Unverified**, **Failed**
or **Not applicable** (shown only in the detailed receipt view). It comes
from recorded [checks](#check), never from what the model claims. See
[Verification](features-integrations-and-safety.md#verification).

## W

### Workspace trust

Your decision that a project folder may use its own settings, hooks, MCP
servers and extensions in full. Until you trust it, the project can only
make things stricter. See [Settings layers and workspace trust](features-integrations-and-safety.md#settings-layers-and-workspace-trust).

### Worktree

A second working copy of your git repository, in its own folder, where an
isolated agent can change files without touching yours. See
[Worktree isolation and merge-back](features-agents-and-context.md#worktree-isolation-and-merge-back).

See also: [Glossary: the desktop app](glossary-desktop.md) ·
[Glossary: experimental features](glossary-experimental.md) ·
[How HyperForm works](README.md) · [Crates](crates.md) ·
[User guide](../user-guide/README.md)
