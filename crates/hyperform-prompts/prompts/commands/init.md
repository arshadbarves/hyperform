---
description: Analyze this repository and create or improve its AGENTS.md
argument-hint: "[focus areas or extra instructions]"
---

Create or improve the `AGENTS.md` file at the project root so that future
agent sessions can work effectively in this repository.

Additional instructions from the user (may be empty): $ARGUMENTS

## Investigate

- Read the guidance that already exists: `AGENTS.md`, `CLAUDE.md`,
  `.cursor/rules/`, `.cursorrules`, `.github/copilot-instructions.md`,
  `README.md`, and contributor documentation.
- Read the build and dependency manifests, task runners (`Makefile`,
  `justfile`, package scripts), CI workflows, and the lint, format, and test
  configuration. Call Verify to list the checks the project exposes.
- Map the layout and the main components. In a large repository, launch
  `explore` agents in parallel for different areas.

## Write

Include only what an agent would otherwise have to rediscover:

- **Commands**: the exact commands to build, test, lint, format, typecheck,
  and run the project, including how to run a single test. Take them from the
  manifests and CI configuration; do not invent them.
- **Architecture**: the big picture that takes several files to understand:
  the main components, how they interact, where key logic lives, and
  important boundaries or dependency rules. Not a file-by-file listing.
- **Conventions**: project-specific rules that differ from common defaults,
  such as code organization, error handling, naming, testing patterns,
  generated files that must not be edited, and commit message style.
- **Gotchas**: non-obvious setup steps, required environment variables (names
  only, never values), and tooling quirks.

Leave out generic advice ("write clean code", "add tests"), anything obvious
from a directory listing, and anything you could not confirm. Keep it tight:
short sections, bullets, and commands in fenced code blocks, usually well
under 200 lines.

If `AGENTS.md` already exists, improve it in place: keep accurate content and
the author's structure, fix what is stale, and fill the gaps. Do not remove
rules the user wrote unless they are clearly wrong, and mention any you
changed. Fold still-accurate guidance from other instruction files into
`AGENTS.md` rather than contradicting it, but leave those files unchanged.

## Finish

Reply with a short summary: whether you created or updated the file, its main
sections, and anything you could not confirm, such as commands you did not
run.
