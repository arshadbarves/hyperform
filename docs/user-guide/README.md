# HyperForm 2.0 user guide

HyperForm is a desktop app that works on your code with you. You describe a
task in plain language ("fix the failing login test", "explain how billing
works"), and an AI model reads files, runs commands, edits code and checks
its own work. Before it does anything that changes your machine, HyperForm
asks for your approval, unless you have told it that action is fine.

HyperForm connects to an AI model provider of your choice (OpenRouter,
Anthropic, OpenAI, OpenCode Zen, or a model running on your own computer).
The model decides what to do; HyperForm carries it out, enforces your
permission rules, records everything, and lets you undo changes.

## What HyperForm can do

- **Understand a codebase.** Ask questions; the agent searches and reads the
  code and answers with file references.
- **Change code safely.** Edits and commands pass through one permission
  gate. You approve them one by one, or write rules that approve them for you.
- **Show its evidence.** Every turn that changes files gets a badge:
  **Verified**, **Unverified** or **Failed**, based on the tests and builds
  that actually ran.
- **Undo.** Before each message you send, HyperForm takes a snapshot of your
  project files. You can rewind the code, the conversation, or both.
- **Delegate.** The agent can start helper agents (subagents) that search,
  plan, review or verify in parallel, optionally in a separate copy of your
  repository.
- **Fit your workflow.** Custom agents, slash commands, skills, hooks and
  `AGENTS.md` instruction files, including existing Claude Code `.claude/`
  folders and `CLAUDE.md` files.

## A short tour of the window

| Area | What it is |
|---|---|
| **Sidebar** (left) | At the top: **New chat** (⌘N / Ctrl+N), **Search** (⌘K / Ctrl+K, the command palette), **Home** and **Inbox**, whose badge counts what is waiting for you. Below: **Projects** (your project folders) with their git branch, number of uncommitted changes, and chats. A dot before a chat means it is working, needs you (amber), finished while you were away, or its last response didn't complete. At the bottom: the model in use, an **Update** button when a new version is ready, and **Settings** (⌘, / Ctrl+,). |
| **Title bar** | In the middle, the **island**: your **pet** (a small creature whose face shows the agent's state; at the default level it also walks onto the composer, the sidebar and the side panel while nothing needs you) and one line saying what the agent is doing, with at most one number (the elapsed time, the finished turn's duration, or a retry countdown). Click it for a card with the live steps, plan, helpers, context, cost and recent warnings. Left of it, an amber count of other chats that need you; right of it, a ring showing how full the context is. At the right, the changes button (**Review changes**, ⌘D / Ctrl+D) with the number of files this chat changed, and the side panel button. Double-click an empty part of it to maximize or restore the window. |
| **Main area** (center) | The project home when no chat is open, the chat's transcript (your messages, the agent's replies with their work folded into one line, its tool calls and a receipt under each turn), or the Inbox. |
| **Composer** (bottom) | Where you type. Above it: queued messages and attachments. In its bar: the **+** menu, the permission mode, the model with its reasoning effort, and Send (it turns into **Stop** while the agent works). An approval card or a question takes its place until you answer. |
| **Side panel** (right) | One panel with four tabs: **Changes**, **Plan**, **Agents** (helpers and jobs) and **Context** (the prompt inspector). Resize it from its left edge or let it use the whole stage. |

The project home shows starter prompts that fit the project and cards for
recent chats, uncommitted changes and project setup. The composer's **+**
menu lists what `@`, `/`, `#` and `!` do.

## Contents

1. [Getting started](01-getting-started.md) – install, first-run setup,
   connect a model provider, open a project, and run your first task.
2. [Everyday use](02-everyday-use.md) – the project home, title bar, your
   pet and the Inbox; asking, fixing, reviewing changes, committing, attaching files,
   steering, rewinding and exporting.
3. [Permissions and safety](03-permissions-and-safety.md) – permission
   modes, approval cards, allow/ask/deny rules, workspace trust and the
   sandbox.
4. [Agents](04-agents.md) – subagents, background agents, worktree
   isolation, and writing your own agents.
5. [Plan mode, questions and todos](05-plan-mode-questions-and-todos.md) –
   reviewing plans before any change, answering the agent's questions, and
   the todo list.
6. [Commands and skills](06-commands-and-skills.md) – every slash command,
   the command palette, writing custom commands, MCP prompts and skills.
7. [Memory and context](07-memory-and-context.md) – `AGENTS.md` files,
   rules, the context window, compaction and prompt caching.
8. [Hooks](08-hooks.md) – run your own scripts on agent events.
9. [MCP and code intelligence](09-mcp-and-code-intelligence.md) – connect
   MCP tool servers and language servers.
10. [Verification](10-verification.md) – badges, checks and verification
    modes.
11. [Models, providers and cost](11-models-providers-and-cost.md) – API
    keys, model roles, reasoning effort, fallbacks and cost tracking.
12. [Settings reference](12-settings-reference.md) – every settings key,
    file and environment variable.
13. [Sessions and data](13-sessions-and-data.md) – where things are stored,
    checkpoints, v1 import and privacy.
14. [Troubleshooting](14-troubleshooting.md) – `/doctor`, the log file and
    fixes for common problems.
15. [Experimental features](15-experimental-features.md) – trying new
    features (Off, Shadow, On), the decision model (sidecar or in the
    app), and how a feature becomes standard.
16. [Decision features](16-decision-features.md) – what each of the 21
    experimental decision features does and what you see when it is on.
17. [Decision settings reference](17-decision-settings.md) – every
    `[decisions]` key, the feature settings and your own decision rules.
18. [Work plans](18-work-plans.md) – plans of tasks with criteria and
    proof, helpers on tasks, parallel tasks, and the plan audit.
19. [Pausing and continuing plans](19-pausing-and-continuing-plans.md) –
    pause, hold or stop a plan, pick it back up after a restart or in
    another chat, and what "needs you" means.

> **Note:** HyperForm 2.0 is a desktop app only. There is no separate
> command-line version.

See also: [Getting started](01-getting-started.md)
