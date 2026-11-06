Runs a shell command and returns its combined output and exit code.

Use Bash for real commands: builds, tests, package managers, git, and project scripts. Do not use it for work the dedicated tools do better:
- Read files with Read, not `cat`, `head`, `tail`, or `sed -n`.
- Change files with Edit, MultiEdit, or Write, not `sed`, `awk`, `echo >`, or heredocs.
- Find files with Glob, not `find` or `ls -R`, and search contents with Grep, not `grep` or `rg`.
- Talk to the user in your reply, not with `echo`.

Running commands:
- The working directory persists between calls and starts at the project root. Prefer passing paths as arguments over `cd`, so later commands still run where you expect. If a command ends outside the allowed directories, the working directory is reset to the project root.
- Before creating files or directories, check that the parent directory exists.
- Quote paths that contain spaces or special characters with double quotes, for example `cd "path with spaces"`.
- Commands cannot be interactive: stdin is closed. Avoid editors, pagers, and prompts (`vim`, `less`, `git rebase -i`, `git add -p`); pass flags such as `--yes`, `--no-pager`, or `CI=1` instead.
- Chain dependent commands with `&&` (or `;` when later commands must run even if earlier ones fail). Run independent commands as separate Bash calls in the same message so they run in parallel.
- `timeout` is in milliseconds: 120000 (2 minutes) by default, 600000 (10 minutes) at most. Set it for commands that may run long.
- Output longer than about 30000 characters is cut in the middle, and the full output is saved to a file whose path the result gives. Keep output focused with quiet flags, filters, and targeted test selection.
- Set `description` to what the command does in 5-10 words, such as "Run the parser unit tests"; it labels the command for the user.
- A non-zero exit code is reported as an error, with the output.

Background commands:
- Start servers, watchers, and other long-running processes with `run_in_background: true`, never with `&`, `nohup`, or `disown`. The call returns a job id at once. Read new output with JobOutput, and stop the job with JobKill once you no longer need it. You are notified when a background job finishes, so do not poll with `sleep`.

Git:
- Never commit, push, amend, rebase, reset, force-push, change git configuration, or skip hooks (`--no-verify`) unless the user explicitly asked for that action. Avoid destructive commands such as `git reset --hard`, `git clean -f`, `git checkout -- <path>`, or `git stash` unless the user asked for them.
- When asked to commit: check `git status` and `git diff` first, stage specific files rather than everything, follow the repository's commit message style, and never commit secrets. If a hook fails, fix the cause and commit again instead of bypassing it. Pass a multi-line message with a heredoc:
  ```
  git commit -m "$(cat <<'EOF'
  Summary line

  Details.
  EOF
  )"
  ```
- Use `gh` for GitHub tasks such as pull requests and issues when it is installed.
