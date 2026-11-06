# Verification

An AI model can say "the tests pass" without running them. HyperForm
doesn't take its word for it: every check the agent runs is recorded as
evidence, and each turn that changed files gets a badge based on that
evidence. This page explains the badges, where checks come from, the
`Verify` tool, the verification modes, and how to add your own checks.

## The badges

The receipt under each turn shows one badge, with the number of checks
behind it. Click it to see the reason and the checks (command, pass/fail or
exit code, test counts, duration).

| Badge | Meaning |
|---|---|
| **Verified** | Files changed, and a passing **test**, **build** or **typecheck** check ran *after* the last change, on exactly the files as they are now. |
| **Unverified** | Files changed, but no such evidence exists. The reason says why: "no checks have run since the last change", "checks are stale: files changed after they ran", or "only lint/format checks ran; run tests or a build". |
| **Failed** | The latest run of at least one check failed (the reason names up to three, with exit codes and failed test counts). |
| **Not applicable** | No files changed in this turn (or verification is off). |

Rules behind the badge:

- Only the latest run of each check counts, and only runs that started
  after the last change.
- Any edit after a check makes it stale. HyperForm compares a fingerprint
  of the project taken after the check with the project now.
- Lint and format checks alone never make a turn Verified; neither do
  `custom` checks.
- A `!command` you ran yourself or an agent's worktree changes you applied
  also count as changes for the next turn.
- The badge never blocks you. It's information.

How much of this the receipt shows depends on **Settings → Appearance →
Task report detail** (`ui.task_report_view`):

| Detail | Badge and checks |
|---|---|
| **Quiet** (default) | The badge only when it is **Verified**, **Unverified** or **Failed**; **Not applicable** is hidden. |
| **Compact** | The same badge, plus the changed files, duration and cost. |
| **Detailed** | Every badge, **Not applicable** included, with the checks already unfolded, plus tokens. |

While checks run at the end of a turn, the island in the title bar says
**Checking the changes**. When the turn ends, it briefly shows the result,
such as **Verified**, **Checks failed** or **Done · not verified** (the
pet cheers or droops), and the receipt keeps the badge.

## Where checks come from

When a chat opens, HyperForm looks for the project's checks by reading
build files. It never runs anything to find them. It scans up to four
folder levels deep and skips dependency, build and cache folders
(`node_modules`, `target`, `dist`, `.venv`, and similar) and anything your
`.gitignore` ignores.

| Ecosystem | Found by | Checks |
|---|---|---|
| Rust (Cargo) | `Cargo.toml` | `cargo test`, `cargo build`, `cargo clippy --all-targets` (with `--workspace` at a workspace root), `cargo fmt --check` |
| Node | `package.json` scripts | Scripts named `test`, `build`, `typecheck`, `type-check`, `tsc`, `check`, `lint`, `format:check`, `fmt:check`, `prettier:check`, `format`, `fmt`, `prettier`, run with npm, pnpm, yarn or bun (chosen by the lockfile or `packageManager`) |
| Deno | `deno.json` / `deno.jsonc` tasks | The same task names via `deno task`, plus `deno test` |
| Python | `pyproject.toml`, `setup.cfg`, `setup.py`, `pytest.ini`, `tox.ini`, `requirements.txt` | pytest, mypy, pyright, ruff, flake8 when configured (for example `[tool.pytest]`, `conftest.py`, a `tests/` folder, `ruff.toml`); run through `uv run`, `poetry run`, the project's `.venv`, or system Python |
| Go | `go.mod` | `go test ./...`, `go build ./...`, `go vet ./...` |
| Gradle | `settings.gradle(.kts)` / `build.gradle(.kts)` | `test`, `build` (using `./gradlew` if present) |
| Maven | `pom.xml` | `mvn -q test`, `mvn -q -DskipTests package` (using `./mvnw` if present) |
| .NET | `.sln`, `.slnx`, `.csproj`, `.fsproj`, `.vbproj` | `dotnet build`; `dotnet test` for test projects |
| CMake | `CMakeLists.txt` with a configured `build/` folder | `cmake --build build`; `ctest --test-dir build` when tests are registered |
| Make | `Makefile` / `makefile` / `GNUmakefile` | Targets `test`, `check`, `build`, `lint` |
| just | `justfile` | Recipes `test`, `check`, `build`, `lint` |

Scripts that watch for changes, rewrite files (`--fix`, `--write`), or are
npm's "no test specified" placeholder are skipped. A formatter script
counts only when it reports instead of rewriting (for example
`prettier --check`). Checks from subfolders get the folder as a prefix in
their id, such as `web/npm:test`.

Check ids look like `cargo:test`, `npm:lint`, `python:pytest`. Discovered
checks time out after 600 seconds.

## The Verify tool

The agent runs checks with the `Verify` tool:

- `list` shows every check with its id, kind, label and command.
- `run` runs one check and reports PASS, FAIL or TIMED OUT, the exit code,
  test counts, duration, the end of the output, and where the full output
  is saved (in the chat's artifacts folder).

Running a check goes through the permission gate like any command, so in
**Ask** mode it asks first unless a rule allows it (for example
`Bash(cargo test:*)`). With the [sandbox](03-permissions-and-safety.md#the-sandbox)
on, checks run inside it. Commands that `Verify` doesn't cover (such as a
single test) are run with Bash; those runs don't count as evidence.

With experimental [work plans](18-work-plans.md) on,
each check the main agent runs with `Verify`, or that `auto` and `strict`
run at the end of a turn, is also linked to the task the main agent is
working on, as evidence for its criteria of the same kind (a `test` run
for a test criterion), and to any task waiting in **Verifying** after a
helper's change came back. The task can be completed only once that
evidence passed on the current code. A helper sent for a task links its
checks to that task; other subagents' checks are recorded but not linked
to a task.

The `verify` subagent runs the relevant checks and reports the results
without fixing anything; the main agent can start it in the background.

## Verification modes

Set the mode in **Settings → Verification → Mode** (`verification.mode`).
The modes only act at the end of a turn that changed files.

| Mode | What happens |
|---|---|
| `off` | Nothing runs, and turns show **Not applicable**. |
| `report` (default) | Only the badge is computed from the checks the agent chose to run. |
| `auto` | If there's no fresh evidence, HyperForm runs the **auto checks** itself. Failures are sent back to the agent, which keeps working to fix them. |
| `strict` | Like `auto`, and the agent is also told to keep going until the badge is Verified. |

`auto` and `strict` are bounded by **Max continuations**
(`verification.max_continuations`, default 3, at most 10): the number of
times a turn may be sent back. When the budget is spent, the turn ends with
its current badge and a notice.

**Auto checks** (`verification.auto_checks`, default `["test"]`) choose
which checks run: check kinds (`test`, `build`, `lint`, `typecheck`,
`format`) or check ids (`cargo:test`, `custom:e2e`). In a project with
several sub-projects, only the checks of the sub-projects that contain the
changed files run.

```toml
[verification]
mode = "auto"
max_continuations = 3
auto_checks = ["test", "typecheck"]
```

Two [experimental decision features](16-decision-features.md#verification)
work here. **Completion check** reads the final message of a turn that
ended Unverified. If it claims the work is done or tests passed, `report`
mode shows **Claimed, not checked** on the receipt; `auto` and `strict`
run the project's test, build and typecheck checks when none of your auto
checks matched, and show the badge only when nothing can run. A claim by
itself never makes a turn Verified. **Check selection** skips auto checks
that the changed files can't affect.

## Custom checks

Add your own checks in **Settings → Verification** (the checks list) or in
a settings file:

```toml
[[verification.checks]]
id = "e2e"
label = "End-to-end tests"
command = "npx playwright test"
kind = "test"
cwd = "web"
timeout_secs = 900

# Replace a discovered check by using its id
[[verification.checks]]
id = "cargo:test"
command = "cargo nextest run"
kind = "test"
```

| Field | Meaning |
|---|---|
| `id` | **Required.** An id without a colon gets the prefix `custom:`, so `e2e` becomes `custom:e2e`. An id with a colon (like `cargo:test`) replaces the discovered check with that id. |
| `label` | Display name; defaults to the id. |
| `command` | **Required.** The shell command. |
| `kind` | `test`, `build`, `lint`, `typecheck`, `format` or `custom` (the default when omitted). Only test, build and typecheck can make a turn Verified. |
| `cwd` | Folder to run in, relative to the project. |
| `timeout_secs` | Default 600. |

A check in a higher settings file replaces a check with the same `id` from
a lower one.

> **Tip:** To run a custom check automatically, list it in `auto_checks` by
> the id you gave it (`auto_checks = ["e2e"]`), by its full id
> (`"custom:e2e"`), or by its kind.

## Untrusted projects

Checks defined in a project's own settings are [withheld](03-permissions-and-safety.md#workspace-trust)
until you trust the workspace, and `auto`/`strict` modes don't run checks
automatically in an untrusted project (the badge reason says "automatic
checks are disabled for untrusted projects"). Discovered checks can still
be listed and run by the agent through `Verify`, with approval.

See also: [Settings reference](12-settings-reference.md#verification) · [Agents](04-agents.md) · [Permissions and safety](03-permissions-and-safety.md)
