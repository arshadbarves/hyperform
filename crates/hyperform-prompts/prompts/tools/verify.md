Lists and runs the project's checks (tests, build, lint, typecheck, format) and records every run as verification evidence.

- `action: "list"` shows the discovered and configured checks with their ids, kinds, and commands.
- `action: "run"` with `check` set to a check id runs that check and reports whether it passed, its exit code, test counts, duration, the end of its output, and where the full output is saved.
- The app badges each turn Verified, Unverified, or Failed from the checks run after your last change. Any later edit makes a result stale, so run the check again after changing code.
- Start with the most targeted check, then widen in proportion to the risk. Use Bash for commands the checks do not cover, such as a single test.
- When a check fails, read the output, fix the cause, and run it again. Never claim that code builds or tests pass without a passing run on the current code.
