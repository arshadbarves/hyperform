# v2 engine architecture: servers, verification and sandbox

The services a session runs around its agent: MCP and language servers,
check verification, and the shell sandbox. The runtime, the tool gate,
persistence, settings, hooks, commands, context and work plans are in
[v2 engine architecture](v2-engine.md).

## MCP and language servers

Each session starts its MCP servers in the background from the effective
settings (project-defined servers only in trusted workspaces). Tools are
registered as `mcp__<server>__<tool>` through the shared gate; a
`tools/list_changed` notification refreshes them. Above 40 MCP tools the
schemas are deferred: a reminder lists the tools and `LoadMcpTools` adds the
named schemas from the next request on. MCP prompts become slash commands.

Language servers start lazily per file type when their binary is installed
(rust-analyzer, typescript-language-server, pyright/basedpyright, gopls,
clangd, or configured ones). The `LSP` tool answers navigation, symbol,
diagnostics and rename-preview queries; after a write to a covered file,
error diagnostics are appended to that tool result when the server has
fresh ones.

## Verification

Checks are discovered per session (Cargo, npm/pnpm/yarn/bun, Python, Go,
Gradle, Maven, .NET, CMake, Make, just, Deno) plus configured
`[[verification.checks]]`; discovery never executes anything. `Verify`
lists and runs them through the normal gate and records a `CheckRecord`
(command, exit code, parsed test counts, duration, output artifact, workspace
fingerprints before and after). A turn that changed files ends `Verified`
only when a passing test, build or typecheck record is newer than the last
change and its fingerprint matches the current tree; a failing latest check
gives `Failed`; otherwise `Unverified` with the reason. `verification.mode`:
`off`, `report` (badge only), `auto` (run the configured checks at the stop
boundary in trusted workspaces and feed failures back), `strict` (keep going
until verified or `max_continuations` is spent). Nothing blocks the user.
With a work plan attached, every recorded check (a `Verify` call or a
harness-run check) is also linked as evidence to the criteria of the same
check kind on the task its agent is working on
(`work::link_check`, [work plans](v2-engine-work.md)).

## Sandbox

`[shell.sandbox]` (off by default) runs agent shell commands, background
shells and checks under macOS seatbelt (`sandbox-exec`) or Linux bubblewrap.
Writes are allowed only in the agent's root (a worktree agent gets its
worktree, not the main project), additional directories, temp directories
and known tool caches; `.git/hooks`, `.git/config` and harness settings files
stay read-only so a command cannot plant code that runs outside the sandbox.
`allow_network = false` blocks outbound traffic except localhost (macOS).
With `auto_allow`, the policy allows sandboxed commands whose writes stay in
bounds without asking; deny rules, ask rules and plan mode still win. When
no backend exists, commands run unconfined, keep asking, and a warning says
why.

See also: [v2 engine architecture](v2-engine.md) ·
[Verification](../user-guide/10-verification.md) ·
[AGENTS.md](../../AGENTS.md)
