# Hooks

Hooks are shell commands that HyperForm runs at fixed moments: when a chat
starts, before a prompt reaches the model, before and after each tool call,
when the agent stops, and more. Use them to enforce policies (block risky
commands), automate chores (format files after edits), add context, or get
notified. The format is compatible with Claude Code hooks.

## Events

| Event | When it runs | Matcher tests | Can it block? |
|---|---|---|---|
| `SessionStart` | A chat opens (new or reopened). | `startup` or `resume` | No. Output becomes context for the next turn. |
| `UserPromptSubmit` | Before your prompt is sent to the model. | – | Yes: the prompt is not sent. Output becomes context. |
| `PreToolUse` | Before a tool runs (after its input is checked). | Tool name | Yes: blocks the call, forces allow/ask/deny, or rewrites the input. |
| `PostToolUse` | After a tool finishes (not when cancelled). | Tool name | No; feedback is appended to the tool result. |
| `Stop` | The main agent ends its response. | – | Yes: the agent keeps working, with your reason (at most 5 times per turn). |
| `SubagentStop` | A subagent ends its response. | – | Yes, like `Stop`. |
| `PreCompact` | Before older history is summarized. | `auto` or `manual` | No. |
| `Notification` | HyperForm needs you: an approval, a question, or a plan to review. | – | No. |
| `SessionEnd` | A chat closes (app quit, chat deleted, reopened). | – | No. |

Tool events also run for tool calls made by subagents.

## Configure hooks

In **Settings → Hooks**, each event has a card where you add, edit and
remove hooks in the settings file chosen under **Saving to** at the top of
the page (**User**, **This project** or **Personal (local)**). Or write
them in a settings file:

```toml
[[hooks.PreToolUse]]
matcher = "Bash"
command = "~/.config/z-engine/hooks/check-command.sh"
timeout_secs = 30

[[hooks.PostToolUse]]
matcher = "Edit|MultiEdit|Write"
command = "./scripts/format-changed.sh"
```

| Field | Meaning |
|---|---|
| `matcher` | A regular expression that must match the **whole** target (tool name, `startup`/`resume`, or `auto`/`manual`). `Edit` doesn't match `NotebookEdit`; use `Edit\|MultiEdit`. Empty or `*` matches everything. Ignored for events without a target. |
| `command` | The shell command to run. It runs in the project folder with the agent's shell. |
| `timeout_secs` | Seconds before the hook is stopped (default 60, at least 1). A timed-out hook shows a warning and the agent continues. |

- Hooks from all settings files run, user file first, then the project file,
  then your personal project file; within a file, in the order written. The
  first hook that blocks or stops ends the chain.
- The Hooks tab shows a matcher field for the four events that use one:
  **Tools** for `PreToolUse` and `PostToolUse`, **Start source** for
  `SessionStart` and **Trigger** for `PreCompact`. Leave it blank to match
  everything; saving keeps the matcher in the file.
- A hook under an unknown event name never runs; a warning names it.
- `/hooks` lists the recent hook runs in the current chat. When a hook
  blocks something, a notice in the title bar's island says so, and the
  [Inbox](02-everyday-use.md#the-inbox) keeps it.

## What the hook receives

The hook gets one JSON object on its standard input:

```json
{
  "session_id": "01K5ZQ8X1B2C3D4E5F6G7H8J9K",
  "transcript_path": "/Users/me/Library/Application Support/z-engine/sessions/01K5ZQ8X1B2C3D4E5F6G7H8J9K/log.jsonl",
  "cwd": "/Users/me/code/shop",
  "hook_event_name": "PreToolUse",
  "permission_mode": "default",
  "tool_name": "Bash",
  "tool_input": { "command": "rm -rf build", "description": "Clean the build folder" }
}
```

Every event includes `session_id`, `transcript_path` (the chat's log file),
`cwd` (the project folder), `hook_event_name` and `permission_mode`
(`plan` while [plan mode](05-plan-mode-questions-and-todos.md) is on,
otherwise `default`, `acceptEdits` or `bypass`). Extra fields per event:

| Event | Extra fields |
|---|---|
| `SessionStart` | `source`: `startup` or `resume` |
| `UserPromptSubmit` | `prompt`: the text you sent |
| `PreToolUse` | `tool_name`, `tool_input` |
| `PostToolUse` | `tool_name`, `tool_input`, `tool_response`: `{ "content": "...", "is_error": false }` |
| `Stop` | `stop_hook_active`: `true` if a stop hook already kept this turn going |
| `SubagentStop` | `agent_id`, `stop_hook_active` |
| `PreCompact` | `trigger`: `auto` or `manual`; `custom_instructions` |
| `Notification` | `message`, for example "HyperForm needs your permission to use Bash" |
| `SessionEnd` | `reason`, for example `shutdown`, `delete` or `close` |

The environment also contains `ZENGINE_PROJECT_DIR` and
`CLAUDE_PROJECT_DIR` (both the project folder), plus the same variables the
agent's shell commands get (see [Settings reference](12-settings-reference.md#shell)).

## What the hook returns

**Exit code:**

| Exit code | Meaning |
|---|---|
| `0` | Continue. For `SessionStart` and `UserPromptSubmit`, plain text on standard output is added as context for the model. |
| `2` | Block, with standard error as the reason (what "block" means depends on the event; see the table above). |
| Anything else | A warning notice shows the error; the agent continues. |

**JSON output:** with exit code 0, standard output that starts with `{` is
read as JSON:

| Field | Effect |
|---|---|
| `decision` | `"block"` blocks like exit code 2 (with `reason`). `"approve"` allows a `PreToolUse` call without asking. |
| `reason` | The reason for `decision`. |
| `continue` | `false` stops: for `Stop`/`SubagentStop` it ends the turn; for `UserPromptSubmit` the prompt isn't sent. |
| `stopReason` | Shown when `continue` is `false`. |
| `hookSpecificOutput.permissionDecision` | `PreToolUse` only: `"allow"`, `"ask"` or `"deny"`. |
| `hookSpecificOutput.permissionDecisionReason` | The reason shown with that decision. |
| `hookSpecificOutput.updatedInput` | `PreToolUse` only: a JSON object that replaces the tool's input. |
| `hookSpecificOutput.additionalContext` | Extra context for the model (`SessionStart`, `UserPromptSubmit`) or feedback appended to the result (`PostToolUse`). |

A hook can make a decision stricter (allow → ask → deny) or allow something
that would ask, but it **can't** override a deny rule or plan mode. When
several hooks decide, the strictest decision wins. For `PostToolUse`, exit
code 2 doesn't undo anything: its message is added to the tool result as
feedback for the agent.

## Examples

These examples use `jq` to read the JSON input; install it or adapt them to
Python.

### Block dangerous commands

```bash
#!/usr/bin/env bash
# ~/.config/z-engine/hooks/check-command.sh
cmd=$(jq -r '.tool_input.command // empty')
if printf '%s' "$cmd" | grep -Eq 'rm -rf /|git push --force|DROP TABLE'; then
  echo "Blocked by policy: $cmd" >&2
  exit 2
fi
exit 0
```

```toml
[[hooks.PreToolUse]]
matcher = "Bash"
command = "~/.config/z-engine/hooks/check-command.sh"
```

The agent receives "A PreToolUse hook blocked this call: Blocked by policy:
..." and must find another way. (For simple cases, a deny rule such as
`Bash(git push --force:*)` does the same without a script.)

### Format files after edits

```bash
#!/usr/bin/env bash
# scripts/format-changed.sh
file=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty')
case "$file" in
  *.ts|*.tsx|*.js|*.json|*.css) npx prettier --write "$file" >/dev/null 2>&1 ;;
  *.rs) rustfmt "$file" 2>/dev/null ;;
esac
exit 0
```

```toml
[[hooks.PostToolUse]]
matcher = "Edit|MultiEdit|Write"
command = "./scripts/format-changed.sh"
```

Because the formatter changes the file after the agent wrote it, the agent
is told the file changed and reads it again before its next edit.

### Get notified

```toml
# macOS: a notification when the agent needs you, and when it finishes
[[hooks.Notification]]
command = '''osascript -e "display notification \"$(jq -r .message)\" with title \"HyperForm\""'''

[[hooks.Stop]]
command = "osascript -e 'display notification \"Task finished\" with title \"HyperForm\"'"
```

On Linux, use `notify-send "HyperForm" "Task finished"` instead.

### Add context to every prompt

```bash
#!/usr/bin/env bash
# scripts/prompt-context.sh — plain output becomes context
echo "Current branch: $(git branch --show-current)"
echo "Changes compared with main: $(git diff main --shortstat)"
```

```toml
[[hooks.UserPromptSubmit]]
command = "./scripts/prompt-context.sh"
timeout_secs = 10
```

## Trust and safety

- Hooks run with your user account's permissions, outside the sandbox, and
  without approval prompts. Only use scripts you understand.
- Hooks defined in a project's `.z-engine/settings.toml` or
  `settings.local.toml` run only after you [trust the workspace](03-permissions-and-safety.md#workspace-trust).
  Hooks in your user settings always run.
- A hook's output is limited to 256 KiB.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Settings reference](12-settings-reference.md#hooks) · [Memory and context](07-memory-and-context.md)
