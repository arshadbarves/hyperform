# Troubleshooting

This page helps you find out what's wrong and fix the most common problems:
start with `/doctor` and the log file, then look up your symptom below.

## Start with /doctor

Type `/doctor` in a chat. It checks, without using the model:

- the provider, its address, and whether an API key is present;
- the model and its context window;
- whether the model catalog is loaded;
- whether git and ripgrep are installed;
- language servers (configured, starting, running or failed);
- MCP servers and their state;
- how many hooks are configured;
- whether the workspace is trusted, and what stays off if not;
- the verification mode and the checks found.

`/status`, `/mcp` and `/context` give more detail on the session, MCP
servers and context use.

## The app log

The log is `hyperform.log` in the data folder:

- macOS: `~/Library/Application Support/z-engine/hyperform.log`
- Linux: `~/.local/share/z-engine/hyperform.log`
- Windows: `%APPDATA%\z-engine\hyperform.log`

For more detail, start HyperForm with the environment variable
`RUST_LOG=debug`. Notices appear briefly in the island at the top of the
window, and errors open it. Click the island for recent warnings and errors
under **Recent**; the **Inbox** keeps every notice in full while the app
runs. That is usually the quickest clue.

## Common problems

### No models are listed in the model picker

- The picker shows only the **active provider's** models. If that provider
  needs a key and has none, the list is empty: add the key in **Settings →
  Providers**.
- A model you expect is missing: the picker hides models the catalog marks
  as retired, and OpenCode Zen without a key lists only its free models.
  Add a Zen key, or type the id in the field at the bottom.
- Local servers (Ollama, LM Studio) and some custom endpoints aren't in the
  catalog. Type the model name in the field at the bottom of the picker
  (**Another model id, e.g. …**) and click **Use**.
- The catalog is downloaded from models.dev the first time you open the
  picker. If you were offline, it retries the next time you open it.
- The cached catalog is refreshed in the background once it is a day old,
  so a model released today may appear only after you open the picker
  again. You can always use a model by typing its id.

### "provider returned HTTP 401" or other key errors

- Check **Settings → Providers**: the active provider should say
  **Active**, not **Active · needs a key**.
- Keys are stored per API host. A custom endpoint or a changed base URL
  needs its own key.
- An environment variable wins over a stored key: `ZENGINE_API_KEY` first,
  then `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` or
  `OPENCODE_API_KEY`. A stale variable can hide the key you stored.
- "stored API keys are unreadable" means `auth.json` is damaged; HyperForm
  won't overwrite it. Fix or remove the file and enter the key again.
- If the model id is wrong for the provider, the provider reports an
  error; check the id format (for example `anthropic/claude-sonnet-4.5` on
  OpenRouter, `claude-sonnet-4-5` on Anthropic).

### OpenCode Zen: "FreeTierError" or "free tier can only be used from within OpenCode"

Zen's free models require the OpenCode client fingerprint (streaming, a
shaped session id, and `shell`/`read` tools). HyperForm sends that for
`opencode.ai` URLs. If you still see this:

- Confirm the base URL is `https://opencode.ai/zen/v1` (not a mirror that
  forwards without the free-tier headers).
- Pick a model tagged **Free** in the model picker (for example
  `big-pickle`). Retired free ids, which the picker hides, return a
  different provider error.
- Paid Zen models need an `OPENCODE_API_KEY` or a key stored under
  OpenCode Zen in **Settings → Providers**.

### GitHub Copilot: sign-in and model errors

- "sign in to GitHub Copilot first (Settings > Providers)" – no token is
  stored for this Copilot address. Click **Configure** next to **GitHub
  Copilot** and **Sign in with GitHub**. GitHub.com and each Enterprise
  domain need their own sign-in.
- "GitHub Copilot refused the token: sign in to GitHub Copilot again
  (Settings > Providers)", followed by "provider returned HTTP 401" or
  "403" – the token was revoked, has no Copilot access, or came from
  another app. Click **Sign in again** in the GitHub Copilot dialog. A
  `ZENGINE_API_KEY` in your environment is sent instead of the stored
  token, so unset it.
- "GitHub Copilot model *X* is disabled for this account: enable it in
  your GitHub Copilot settings" – turn the model on in your Copilot
  settings on GitHub (or ask your organization's admin), or pick another
  model.
- "The code expired before you approved it. Sign in again for a new
  code." – click **Sign in with GitHub** again and approve sooner.

### Rate limits, "Provider busy", retrying

HyperForm retries temporary failures (HTTP 408, 429, 500, 502, 503, 504,
529, timeouts, lost connections) up to 5 attempts, while the island shows
**Provider busy** and a countdown such as **retry 2 in 4s**. If it still
fails, the turn ends with the provider's error.

- Wait and send again, or choose another model.
- Configure `model.fallbacks` so another model takes over automatically
  ([Models, providers and cost](11-models-providers-and-cost.md#fallback-models-and-retries)).
- Many parallel subagents use the provider heavily; lower
  `agents.max_concurrent`.

### A tool was denied

Expand the tool card (unfold its run first if it shares one line with
other calls); it says why:

- "denied by rule ..." – a deny rule in one of your settings files.
- "plan mode is read-only" – turn plan mode off (Shift+Tab, or the **×**
  on the **Plan** chip) or build the plan. From a subagent such as
  `explore`, `plan` or `review`, the agent is read-only by design, even in
  [Bypass](03-permissions-and-safety.md#plan-mode): ask for one that may
  make changes (`@agent-general`).
- "A PreToolUse hook blocked this call" – a hook refused it; `/hooks`
  shows recent hook runs.
- "The user denied this action" – someone answered **Deny**.
- "this agent works in the isolated worktree ..." – a worktree agent tried
  to change your main project.

**Settings → Permissions** shows all rules from all files. Rules that don't
parse are skipped with a "ignored permission rule" notice.

### An MCP server fails

- Run `/mcp`: it shows the error and the last lines of the server's error
  output.
- Use **Test** in **Settings → MCP**.
- "not found on PATH": use the full path of the program in `command`.
- The server doesn't see your shell's variables; put tokens in `env`
  ([details](09-mcp-and-code-intelligence.md#in-a-settings-file)).
- A server defined in the project's settings doesn't start until you
  [trust the workspace](03-permissions-and-safety.md#workspace-trust).
  `/mcp` says so.
- Slow servers: raise `timeout_secs`.

### A language server doesn't start

- Install the server program and make sure it's on the `PATH` HyperForm
  sees (for example `rust-analyzer`, `typescript-language-server`,
  `pyright`, `gopls`, `clangd`).
- Servers start on the first file the agent asks about; `/doctor` lists
  "none started yet" until then.
- Check `lsp.enabled` and that the server isn't disabled in
  `[lsp.servers]`.
- For rust-analyzer and gopls, the project needs its root file
  (`Cargo.toml`, `go.mod`).

### "The command sandbox is enabled but unavailable"

- Linux: install bubblewrap (`bwrap`) from your package manager.
- macOS: the built-in `sandbox-exec` can't start inside another sandbox
  (for example when HyperForm itself runs sandboxed).
- Windows: there is no sandbox yet.

Until it's fixed, commands run unconfined and ask for approval.

### Commands fail with "command not found"

Commands only get a few environment variables, and apps started from the
Dock, Start menu or a launcher often have a shorter `PATH` than your
terminal. Set the path explicitly:

```toml
[shell.env]
PATH = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
```

Add other variables your tools need to `shell.env_passthrough`, for
example `["SSH_AUTH_SOCK", "JAVA_HOME"]`. To use another shell, set
`shell.path`.

### "The decision model is not used"

The full notice is "The decision model is not used: *reason*.
Experimental decisions keep today's behavior." It appears when a chat
opens, or its settings change, while an
[experimental decision feature](15-experimental-features.md#the-decision-model)
runs in Shadow or On and the decision model can't be set up. Nothing
breaks: those features keep today's behavior. The reason says what to
fix:

- "its sidecar did not start (...)" – check `decisions.sidecar.command`.
  The sidecar gets the same limited environment as the agent's commands,
  so use the program's full path or set `shell.env`
  ([above](#commands-fail-with-command-not-found)).
- "decisions.api_key_env names ..., which is not set" – set that variable
  before you start HyperForm, or remove `api_key_env`.
- "the decision endpoint ... is not on this machine; set
  decisions.allow_remote to use it" – use an address on this computer, or
  turn on `allow_remote` if you mean it.
- "invalid decision endpoint ..." – use a full `http://` or `https://`
  address without a user name or password in it.

With **Runtime** set to **In the app** (`decisions.runtime = "native"`):

- "this app was built without the native runtime (the `onnx` feature);
  use the sidecar runtime" – this build can't run the model itself (the
  release installers can't yet). Set **Runtime** back to **Sidecar**.
- "the native model for checkpoint "*X*" is not downloaded; download it in
  Settings (Experimental, Decision model)" – click **Download** (or
  **Resume**) in the **Native model** row.
- "the native runtime has no model for checkpoint "*X*" (it has english,
  multilingual, typed-decisions)" – set **Checkpoint** to one of those.

The in-app model loads in the background; while it loads, questions fall
back ("the native model is still loading"), and **Test connection** may
say "No answer yet". If it says "... of the native model does not match
its pinned SHA-256; remove the model in Settings and download it again",
do that: click **Remove**, then **Download**.

A server that is set up but doesn't answer isn't checked when the chat
opens. Each of its questions falls back instead, and the Context tab's
**Decisions** section says "fell back: unavailable" or "fell back:
timeout".

**Test connection** on the Decision model card says "Not connected:" with
the same reasons, or with what went wrong on the way: "the decision server
is unreachable" (nothing answers at that address, or the sidecar is still
starting), "the decision server answered HTTP 401" (a wrong or missing
key), or "did not answer within 10000 ms". A slow answer, or a wrong
answer to its test question, gets its own advice; see
[Test the connection](15-experimental-features.md#test-the-connection).

### The first request is slow

On the first message of a chat, HyperForm takes the first code checkpoint
(hashing every project file; the agent's tools wait for it), builds the
repository map, and starts MCP servers. In large projects this can take a
while; later messages are faster, and prompt caching makes repeated
context cheaper and quicker. Projects with more than 50,000 files skip
checkpoints.

### I edited a file by hand and nothing changed

Settings changed in the Settings screens apply to open chats at once.
Files you edit by hand (settings, agents, commands, skills, rules,
instructions) are read when a chat opens; start a new chat or restart
HyperForm. A settings file with an error is skipped: its Settings page shows
"This settings file is not applied until it is fixed".

### The agent stopped by itself

The note on the turn's receipt or a notice explains why:

- "Stopped · reached the limit of 200 model turns" – raise
  `agents.max_turns`.
- "Stopped · reached the session cost cap" – raise or clear
  `agents.session_cost_cap_usd`.
- "Verification ended after N automatic continuation(s)" – auto/strict mode
  used its budget.
- "Interrupted · the app closed mid-turn" – send your request again.

### A work plan refuses a change

With [work plans](18-work-plans.md) on, every change
to a plan is checked first. A refused change alters nothing; the agent's
tool result starts with "The plan refused this change:", and your own
change shows a notice such as "The plan change was not applied:", followed
by each reason. When the agent sends a helper for a task, the refusal
starts with "Task … cannot be dispatched:" instead.

| Reason | What to do |
|---|---|
| "... needs a passing test check on the current tree: no check has been linked" (or "the latest check ran on an older tree", "the latest check on this tree failed") | The agent must run `Verify` after its last edit, while the task is in progress. A test run with `Bash` doesn't count. If the criterion doesn't apply, **Waive** it in the task's detail. |
| "... criterion ... is not met" | An inspection criterion the agent hasn't confirmed yet, or one you check yourself: click **Confirm** once you have checked it. |
| "task ... cannot start: the plan is draft" (or "replanning", "paused") | Click **Build**, **Accept changes** or **Resume** in the **Plan** tab (**Reopen** for a done or abandoned plan). |
| "task ... cannot start: it is held until the plan change is accepted" | A problem found in your files touches this task. Review the changes and click **Accept changes**; the other tasks keep running meanwhile. |
| "task ... cannot start: the user holds it" | You held the task: **Release** it in its **⋯** menu. |
| "Task … cannot be resumed: …" | Only the interrupted helper itself can be continued, from the chat it ran in. The agent can start a new helper for the task instead; the earlier helper's work is then not used. |
| "task ... waits on unfinished dependencies" | Finish those tasks first, or mark them **Not needed**. |
| "task ... was already started 3 times (the limit is 3)" | The task hit `work.max_attempts`; see why it keeps failing, then change the task or raise the limit ([Work settings](12-settings-reference.md#work)). |
| "only the user can ..." | That change is yours to make in the **Plan** tab, such as waiving a criterion, accepting the plan, or accepting a task proposed from web content. |
| "this turn read web content, so it cannot ..." | The agent read a web page this turn, so it may only add tasks and notes that wait for you ([why](18-work-plans.md#what-changes-for-the-agent)). Make the change yourself in the **Plan** tab, or ask again in a new message that doesn't need the web. |
| "no work plan is attached to this chat; TaskCreate starts one" | The chat has no plan yet; the agent's first `TaskCreate` starts one. |
| "Work plans are off; turn on work_plans in Settings > Experimental." | Turn the feature on. |
| "a helper already works on ...; more than one task-bound helper at a time needs work_parallel on and the plan's parallel work approved" | Wait for the helper, or turn on [parallel tasks](18-work-plans.md#parallel-tasks) and allow them for this plan. |
| "task ... cannot run in parallel: ..." | The reason says what links the tasks, such as "its scope overlaps task t2" or "task t3 declares no files" ([which tasks run together](18-work-plans.md#which-tasks-run-together)). Wait for the other task, or change the scopes. |
| "helpers already work on ..., the most allowed at once (work.max_parallel 3)" | Wait for one to finish, or raise `work.max_parallel`. |

If a plan can't be saved (for example the disk is full), it turns
read-only: "the plan could not be saved (...); it is read-only until it is
opened again". Fix the cause, then restart HyperForm. "The work plan of
this chat could not be opened: ..." when a chat opens means its folder in
`plans/` is missing or unreadable
([where it lives](13-sessions-and-data.md#where-things-live)).

### A helper's task stopped or won't run in parallel

- **"ended without a report"**: the helper's run ended before it sent its
  result. Its changes, if any, are listed in the task's **Result**. Click
  **Retry** in **Needs you**, or **Not needed**.
- **"its change was discarded"**: you discarded the helper's change.
  **Retry** runs the task again.
- **"Interrupted"**: the helper stopped before it finished; the note says
  why ([the notes](19-pausing-and-continuing-plans.md#after-a-restart-or-when-a-chat-opens)).
  The task waits with its worktree kept, and the agent continues it
  while the plan is building. **Apply** on such a helper is refused ("This
  helper's work belongs to task …"): let the task resume, or **Discard**.
- **"This plan moved to another chat."**: you continued the plan in
  another chat, so this one's helpers stopped. Open that chat, or
  **Continue a plan** here to move it back.
- **Merge conflict**: applying the change would conflict with your files,
  so nothing was changed. **Retry serially** runs the task again alone on
  your current code; or **Show worktree**, resolve the files, and
  **Apply** again in the **Agents** tab.
- **The task failed after applying**: "the integration check … failed in
  the project tree" means its checks failed once the change was in your
  project; **Retry** it once you know why. "Task …'s integration check
  did not run: …" means a check couldn't run, so the task stays
  **Verifying**: fix the reason, then click **Run checks again** in the
  task's detail or in **Needs you**; **Waive** the criterion if it
  doesn't apply.
- **No Build with helpers in parallel** in the draft's menu: turn on
  **Parallel tasks** (it needs **Work plans**), and make sure at least two
  ready tasks name their files and share no files, symbols, serial paths
  or dependency path. Hover a task's badge to see why it runs alone.

### Rewind is greyed out

"No code checkpoint for this prompt" means checkpoints were off: git isn't
installed, the project has more than 50,000 files, or the snapshot failed
(a notice said why). **Conversation only** still works.

### Keys don't answer the approval card

The `y`/`s`/`p`/`n` keys work when the card has focus; their hints appear
on the card then. Click the card (not a button), then press the key.

## FAQ

**Can I use HyperForm offline?** With a local model server (Ollama, LM
Studio), yes. Web tools, catalog download and update checks need a network.

**Does it use my Claude Code setup?** It reads `.claude/agents`,
`.claude/commands`, `.claude/skills` and `CLAUDE.md` files. It does **not**
read `.claude/settings.json`; put permissions and hooks in
`.z-engine/settings.toml`.

**Does HyperForm 2.0 change my v1 settings or chats?** No. v1 files are only
read. See [Settings reference](12-settings-reference.md#importing-v1-settings).

**Can the agent push to my remote?** Only if you allow the command. The
agent is instructed never to push unless you ask; add
`ask = ["Bash(git push:*)"]` to be sure.

**How do I stop everything?** Esc cancels the turn and its foreground
subagents. Background jobs and agents are stopped with **Stop** under
**Jobs** in the side panel's **Agents** tab. Quitting the app stops all of
them.

**Where did the Review panel, the Terminal and Context & Memory go?** They
were renamed. The Review panel is the side panel's **Changes** tab (⌘D /
Ctrl+D), the Terminal panel is the **Shell** drawer above the composer,
Context & Memory is its **Context** tab, the prompt inspector (**Inspect
prompt** in the context card), and the Now card is the island's card
(click the island in the title bar). The agents panel is the **Agents**
tab, and a plan is reviewed in the **Plan** tab.

**Is there a command-line version?** No. HyperForm 2.0 is a desktop app.

See also: [Getting started](01-getting-started.md) · [Settings reference](12-settings-reference.md) · [Sessions and data](13-sessions-and-data.md) · [Experimental features](15-experimental-features.md)
