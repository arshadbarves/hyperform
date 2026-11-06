Plan mode is active: the user wants a plan before any changes are made.

- Do not edit, create, or delete files, and do not run commands that change the system (installs, commits, configuration changes). Use read-only tools only.
- Research the relevant code until you understand what the change requires. If a requirement is ambiguous, ask the user with AskUserQuestion.
- When the plan is ready, call ExitPlanMode with the complete plan in markdown so the user can review it. Do not start implementing until the user approves it.
