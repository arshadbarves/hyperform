---
description: Review the current changes or a given target for bugs, regressions, security issues, and missing tests
argument-hint: "[files, commit, range, or branch]"
---

Review code changes for correctness, regressions, security problems, and
missing tests. Report findings; do not change code.

Target: $ARGUMENTS

If the target is empty, review all uncommitted changes: staged, unstaged, and
untracked files. Otherwise the target names what to review: files or
directories, a commit, a commit range, or a branch to compare with its merge
base.

## Steps

1. Establish the scope with git (`git status`, `git diff`, `git diff --staged`,
   `git show <commit>`, or `git diff <base>...HEAD`). If there is nothing to
   review, say so and stop.
2. Launch the `review` agent with a self-contained prompt: the exact target
   and how to see its diff, what the change is meant to accomplish (taken from
   this conversation, which the agent cannot see), and any areas of concern.
   For large changes, split the work by area across several `review` agents
   launched in the same message.
3. Check every finding against the code before you report it. Drop false
   positives, merge duplicates, and add real problems the agents missed.

## Report

Start with a one-line verdict. Then list findings from most to least severe:

- **[severity]** `path/to/file:line`: the problem, the concrete scenario in
  which it breaks, and a suggested fix.

Severities: **critical** (security hole, data loss, crash, or broken build),
**high** (wrong behavior users will hit, or a regression), **medium**
(edge-case bug, error-handling gap, or missing test for changed behavior), and
**low** (a minor issue with a real, if small, risk). Skip style nits that
tooling handles.

If there are no significant issues, say so and summarize what you reviewed.
End by offering to fix the findings.
