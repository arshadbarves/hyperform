---
description: Security-focused review of pending changes
argument-hint: "[files, commit, range, or branch]"
---

Perform a security review of pending code changes. Report findings; do not
change code.

Target: $ARGUMENTS

If the target is empty, review all uncommitted changes: staged, unstaged, and
untracked files. Otherwise review the files, commit, range, or branch it
names. Focus on what the change introduces or affects, and read the
surrounding code as far as needed to trace data flow. This is not a full audit
of the repository.

## Method

1. Get the diff with git and list the trust boundaries it touches: network,
   IPC, and command-line input; files and paths; environment variables;
   databases and queries; templates and HTML; process execution;
   deserialization; authentication and sessions; cryptography; secrets; and
   dependencies.
2. For each boundary, trace untrusted input from where it enters to where it
   is used, and check the validation, encoding, and authorization along the
   way. Account for protections the framework already provides.
3. For a large diff, hand parts of the trace to `review` or `explore` agents
   with self-contained prompts, and confirm their findings yourself.

Look especially for:

- injection into SQL, shell commands, templates, paths, or headers, and
  cross-site scripting;
- path traversal and unsafe file handling, including symlinks and archive
  extraction;
- missing or bypassable authentication and authorization, including direct
  object references that skip ownership checks;
- secrets or personal data exposed in code, logs, or error messages;
- weak or misused cryptography, predictable randomness, and disabled
  certificate verification;
- unsafe deserialization, server-side request forgery, CSRF, and permissive
  CORS;
- race conditions with security impact, and memory-safety issues in `unsafe`
  or native code;
- resource exhaustion reachable by untrusted input;
- risky new dependencies or install scripts, and insecure defaults.

Report only issues with a plausible path to exploitation. Leave out
theoretical concerns, generic hardening advice, and problems confined to test
code. Describe exploitation only as far as needed to justify the fix; do not
write working exploits.

## Report

Start with a one-line summary. Then list findings from most to least severe:

- **[severity, confidence]** `path/to/file:line`: title. Attack: who can
  trigger it and how. Impact: what they gain. Fix: the concrete remedy.

Severity is critical, high, medium, or low. Confidence is high or medium;
drop findings you hold with low confidence. Close with what you reviewed and
anything you could not assess. If you found nothing significant, say so
explicitly.
