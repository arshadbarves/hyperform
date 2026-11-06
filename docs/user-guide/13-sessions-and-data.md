# Sessions and data

This page explains where HyperForm keeps your chats, settings, keys and
logs on each operating system, how code checkpoints and rewind work, how v1
chats are imported, and what is sent to your model provider.

## Where things live

HyperForm uses two folders: a **config folder** for settings, keys and your
own extensions, and a **data folder** for chats, checkpoints and logs.

| | Config folder | Data folder |
|---|---|---|
| macOS | `~/.config/hyperform` | `~/Library/Application Support/hyperform` |
| Linux | `~/.config/hyperform` | `~/.local/share/hyperform` (or `$XDG_DATA_HOME/hyperform`) |
| Windows | `%APPDATA%\hyperform` | `%APPDATA%\hyperform` |

`HYPERFORM_CONFIG_DIR` and `HYPERFORM_DATA_DIR` move them. **Settings → About &
Updates → Files & storage** shows the actual paths with copy buttons.

**Config folder:**

| Path | Contents |
|---|---|
| `settings.toml` | Your user settings. |
| `auth.json` | Stored API keys (owner-only permissions on macOS/Linux). |
| `trust.json` | Trusted workspace folders. |
| `AGENTS.md` | Your instructions for every project. |
| `agents/`, `commands/`, `skills/`, `rules/`, `output-styles/` | Your extensions for every project. |
| `models.json` | Optional model catalog additions. |

**Data folder:**

| Path | Contents |
|---|---|
| `sessions/<chat id>/` | One folder per chat (see below). |
| `checkpoints/` | Shadow git repositories for code checkpoints, one per project. |
| `plans/<project key>/<plan id>/` | One folder per [work plan](18-work-plans.md) (empty unless you use them), under the same project key as the project's checkpoints. `events.jsonl` holds every change to the plan in order, including refused ones; `meta.json` its summary (goal, status, progress, what needs you, and the chat it was last in), which the plan list, the home card and the Inbox read. |
| `cache/models-dev.json` | The cached model catalog. |
| `decisions/dataset/` | Only when `decisions.record_dataset` is on: one `<question>.jsonl` file per decision question, a line per answer with the question's full input, which can include text from your chats ([Record decisions](15-experimental-features.md#record-decisions)). |
| `models/laya/<revision>/` | Only after you download the decision model on **Settings → Experimental** ([Run it in the app](15-experimental-features.md#run-it-in-the-app-native)): its files, each checked against a pinned SHA-256 checksum. An unfinished file is kept as `<name>.part` so the download can resume. **Remove** on the card deletes the folder. |
| `workspaces.json` | The project folders listed under **Projects** in the sidebar. |
| `pet.json` | Your [pet](02-everyday-use.md#your-pet)'s growth: XP, day streak, counts of turns, verified turns and applied changes, what it wears, and the last 200 turns and applies it counted (so none counts twice). If it is missing or unreadable, the pet starts fresh. |
| `hyperform.log` | The app log. |

**In each project:**

| Path | Contents |
|---|---|
| `AGENTS.md`, `AGENTS.local.md` (and `CLAUDE.md`, `CLAUDE.local.md`) | Instructions. |
| `.hyperform/settings.toml`, `.hyperform/settings.local.toml` | Project and personal settings. |
| `.hyperform/agents/`, `commands/`, `skills/`, `rules/`, `output-styles/` | Project extensions. |
| `.hyperform/worktrees/` | Worktrees of isolated agents and of chats started with **New chat in a worktree…**. A plan task's worktree, an [interrupted](19-pausing-and-continuing-plans.md#after-a-restart-or-when-a-chat-opens) task's too, stays until its change is applied, discarded or [retried serially](18-work-plans.md#when-a-change-conflicts); opening a chat never deletes one. |

The [Inbox](02-everyday-use.md#the-inbox) is not saved anywhere: it lives
in memory while the app runs, keeps up to 200 notices, and starts empty the
next time you open HyperForm.

### Inside a chat folder

| File | Contents |
|---|---|
| `log.jsonl` | Every event of the chat in order: messages, turns, approvals, questions, plans, todos, agents, checks, checkpoints, compactions, rewinds, mode/model changes, usage, and which work plan is attached. |
| `meta.json` | Title, project, dates, message count, cost. Rebuilt from the log if missing. |
| `agents/<agent id>.jsonl` | Each subagent's transcript. |
| `artifacts/` | Full outputs that were too long for the chat: long command output, cleared tool results, check logs, and the exchanges [task-scoped history](16-decision-features.md#task-scoped-history) set aside (`task-view-turn-N.txt`). |

Chats are written as they happen, so they survive a crash. If the app
closes in the middle of a turn, that turn shows **Interrupted · the app
closed mid-turn** when you reopen the chat; damaged lines at the end of a
log are skipped with a notice.

Deleting a chat (trash icon, then **Delete chat** to confirm) removes its
folder (and its v1 file). It does not remove code checkpoints, which are
shared by all chats of a project, or work plans, which belong to the
project.

## Checkpoints and rewind

Before each message you send, HyperForm snapshots your project's files into
a *shadow* git repository in the data folder's `checkpoints/`. It never
touches your own repository, index or configuration, and it captures
changes made by shell commands too, not only by the agent's edit tools.
The agent's first tool calls in a turn wait until the snapshot is taken.
With [work plans](18-work-plans.md) on, the plan also takes one after
each turn of the main agent and each helper change you apply, when the
files changed, so it can tell later what changed outside the plan.

What a checkpoint leaves out:

- `.git` folders and `.hyperform/worktrees/`;
- everything your repository ignores (`.gitignore` and similar);
- files larger than 8 MiB.

Checkpoints need git installed. For projects with more than 50,000 files
(after ignores), checkpoints are turned off and a notice says so;
`/status` shows whether they're on.

**Rewinding** (**Rewind** at the end of a turn) offers *Code and
conversation*, *Conversation only* or *Code only*
([Everyday use](02-everyday-use.md#rewind)). Restoring code:

1. takes a snapshot of the current state first;
2. rewrites only the files that differ: changed or deleted files come back,
   files created since are deleted (with folders they leave empty);
3. leaves alone any file that a snapshot couldn't store (ignored or too
   large at either time), and lists it in the notice.

Rewinding the conversation records the rewind in the log and drops later
messages from what you and the model see. The agent forgets which files it
had read, so it reads them again before editing.

The Changes tab's **This chat** scope compares the project with the
chat's first checkpoint.

> **Warning:** Rewind can't undo effects outside your project files:
> installed packages, database changes, network calls, `git push`.

## Privacy: what leaves your computer

**Sent to your model provider** with each request:

- the system prompt, which includes your instruction files and rules, the
  repository map (names of functions, types and files), the project folder
  path, platform, shell, date, and a git snapshot (branch, `git status`
  output, recent commit messages);
- the tool definitions, including any MCP tools;
- the conversation: your messages, attached files and images, the agent's
  replies, and the results of its tool calls, such as the contents of files
  it read and command output;
- with work plans on, reminders about the attached plan and its task
  packets: the goal, tasks, criteria, check results and notebook entries.

The fast model also receives your first message (to write a title), older
history (for compaction summaries), and web pages the agent fetched (to
answer its question about them). With work plans on, the review model
receives the plan (goal, document, requirements, tasks and notebook
entries) when the [plan audit](18-work-plans.md#the-plan-audit) runs.

**Sent elsewhere:**

- Web searches go to your search provider (Brave, Tavily, Exa or your
  SearXNG), and WebFetch requests go to the sites being fetched.
- MCP servers receive the arguments of the tool calls made to them.
- The [decision model](15-experimental-features.md#the-decision-model),
  if you turn on a decision feature, receives its questions: short digests
  of your request, the agent's recent steps and tool output. A sidecar
  must be on this computer unless you turn on `decisions.allow_remote`;
  the in-app model sends nothing. Downloading the in-app model fetches
  its files from Hugging Face.
- [Secret screening](16-decision-features.md#secret-screening), when On,
  asks before likely credentials in tool output are sent to your model
  provider; side requests to the fast model are not screened.
- HyperForm downloads the model catalog from `models.dev` when it has none
  cached or the cached copy is a day old, and checks GitHub for updates
  when it starts.

**Stays on your computer:** your chats, logs and artifacts, checkpoints,
work plans, settings, keys (except each key going to its own provider), trust
decisions, and the decision dataset. HyperForm has no account and sends no
usage data of its own.

> **Tip:** To keep secrets away from the model, deny them:
> `deny = ["Read(./.env)", "Read(~/.ssh/**)"]`. A deny rule for `Read`
> also stops shell commands from reading those files.

See also: [Everyday use](02-everyday-use.md) · [Settings reference](12-settings-reference.md) · [Troubleshooting](14-troubleshooting.md) · [Experimental features](15-experimental-features.md)
